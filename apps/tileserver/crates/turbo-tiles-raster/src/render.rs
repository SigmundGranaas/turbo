//! Server-side raster tile rendering: the same PostGIS layers and the same
//! `n50-topo` style as the vector basemap, rasterised with tiny-skia. This
//! is the M1 "raster fallback" — a drop-in XYZ source for `flutter_map`
//! while the on-device vector renderer matures. One source of truth: a
//! style change recolours both pipelines.
//!
//! Per tile: for each style layer active at `z` (style order = paint
//! order), fetch the matching basemap-config layer's geometry as WKB in
//! EPSG:3857 (bbox-filtered + zoom-simplified in 25833, like the MVT path),
//! filter rows by the style layer's attribute filter, project to pixels,
//! draw. Labels render last with an embedded DejaVu Sans and a naive
//! keep-out so tiles don't turn into word soup.

use ab_glyph::{Font, FontRef, ScaleFont};
use geo::Geometry;
use geozero::ToGeo;
use serde_json::Value as Json;
use tiny_skia::{
    FillRule, LineCap, LineJoin, Paint as SkPaint, PathBuilder, Pixmap, PremultipliedColorU8,
    Stroke, Transform,
};
use turbo_tiles_core::tile::TileCoord;
use turbo_tiles_db::DbPool;
use turbo_tiles_mvt::{BasemapConfig, BasemapLayer, GeomKind};

use crate::style::{PaintKind, RasterStyle, Rgba};

/// Half the web-mercator world circumference, metres.
const WORLD_M: f64 = 20_037_508.342_789_244;
/// Extra ring fetched around the tile so strokes/labels crossing the edge
/// don't visibly clip.
const PAD_PX: f64 = 24.0;

pub(crate) const FONT_BYTES: &[u8] = include_bytes!("../assets/DejaVuSans.ttf");

/// Per-render statement-timeout budget (ms), applied via `SET LOCAL` to the
/// render transaction only — so a cold low-zoom tile (whose per-layer
/// `ST_Transform`/`ST_Simplify` over a country-sized bbox can exceed the pool's
/// default 10s cap) can complete *once* and be cached, without lifting the cap
/// for routing/other queries. Env `TILESERVER_RASTER_RENDER_TIMEOUT_MS`
/// (default 60s). 0 disables the override (inherit the connection's cap).
fn render_statement_timeout_ms() -> u64 {
    use std::sync::OnceLock;
    static V: OnceLock<u64> = OnceLock::new();
    *V.get_or_init(|| {
        std::env::var("TILESERVER_RASTER_RENDER_TIMEOUT_MS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(60_000)
    })
}

#[derive(Debug, thiserror::Error)]
pub enum RasterError {
    #[error(transparent)]
    Db(#[from] sqlx::Error),
    #[error("render: {0}")]
    Render(String),
}

/// Web-mercator bounds of a tile, `(xmin, ymin, xmax, ymax)` in metres.
pub fn tile_envelope_3857(coord: TileCoord) -> (f64, f64, f64, f64) {
    let n = f64::from(1u32 << coord.z);
    let side = 2.0 * WORLD_M / n;
    let xmin = -WORLD_M + f64::from(coord.x) * side;
    let ymax = WORLD_M - f64::from(coord.y) * side;
    (xmin, ymax - side, xmin + side, ymax)
}

/// Projection from EPSG:3857 metres into tile-local pixels.
pub(crate) struct Proj {
    xmin: f64,
    ymax: f64,
    scale: f64, // px per metre
}

impl Proj {
    fn new(env: (f64, f64, f64, f64), size_px: u32) -> Self {
        Self {
            xmin: env.0,
            ymax: env.3,
            scale: f64::from(size_px) / (env.2 - env.0),
        }
    }
    fn px(&self, x: f64, y: f64) -> (f32, f32) {
        (
            ((x - self.xmin) * self.scale) as f32,
            ((self.ymax - y) * self.scale) as f32,
        )
    }
}

/// Render one styled raster tile as PNG bytes.
pub async fn render_tile(
    pool: &DbPool,
    cfg: &BasemapConfig,
    style: &RasterStyle,
    coord: TileCoord,
    size_px: u32,
    dem: Option<&turbo_tiles_elev::Dem>,
) -> Result<Vec<u8>, RasterError> {
    let env = tile_envelope_3857(coord);
    let proj = Proj::new(env, size_px);
    let pad_m = PAD_PX / proj.scale;
    let fetch_env = (env.0 - pad_m, env.1 - pad_m, env.2 + pad_m, env.3 + pad_m);
    // Metres per pixel at this zoom drives the same zoom-scaled
    // simplification the MVT path uses.
    let simplify_tol_m = 0.5 / proj.scale;

    let mut pixmap =
        Pixmap::new(size_px, size_px).ok_or_else(|| RasterError::Render("pixmap alloc".into()))?;
    let bg = style.background;
    pixmap.fill(tiny_skia::Color::from_rgba8(bg.r, bg.g, bg.b, bg.a));

    let font = FontRef::try_from_slice(FONT_BYTES)
        .map_err(|e| RasterError::Render(format!("font: {e}")))?;
    // Labels draw after all geometry; collect while walking layers.
    let mut labels: Vec<(f32, f32, String, f32, Rgba)> = Vec::new();

    // Hillshade is composited over the fills (and paper), under the lines, so
    // contours/roads/labels stay crisp. Precompute the relief once when a DEM
    // artifact is loaded; skip entirely otherwise (dev/no-artifact renders
    // flat, same as before).
    let hs = crate::hillshade::HillshadeParams::default();
    let shade = dem.and_then(|d| {
        let px_m = ((env.2 - env.0) / size_px as f64) as f32;
        crate::hillshade::sample_grid(d, env, size_px)
            .map(|grid| crate::hillshade::intensity(&grid, size_px, px_m, &hs))
    });
    let mut shade_applied = false;

    // All layer fetches run in one transaction so a single `SET LOCAL
    // statement_timeout` covers the render without leaking the raised cap back
    // to the pooled connection. Read-only; we roll back at the end.
    let mut tx = pool.begin().await?;
    let budget_ms = render_statement_timeout_ms();
    if budget_ms > 0 {
        sqlx::query(&format!("SET LOCAL statement_timeout = {budget_ms}"))
            .execute(&mut *tx)
            .await?;
    }

    for layer in &style.layers {
        if coord.z < layer.min_zoom || coord.z > layer.max_zoom {
            continue;
        }
        let Some(def) = cfg.layer.iter().find(|l| l.name == layer.source_layer) else {
            continue; // style ahead of config — tolerated, basemap test guards it
        };
        // First non-fill layer: composite the relief now, so it tints the
        // fills below without dulling the lines/labels above.
        if !shade_applied && !matches!(layer.paint, PaintKind::Fill { .. }) {
            if let Some(s) = &shade {
                crate::hillshade::composite(&mut pixmap, s, hs.strength);
            }
            shade_applied = true;
        }
        let rows = fetch_layer(&mut tx, def, fetch_env, simplify_tol_m).await?;
        for (wkb, attrs) in &rows {
            if !layer.filter.matches(attrs) {
                continue;
            }
            let Ok(geom) = geozero::wkb::Wkb(wkb.clone()).to_geo() else {
                continue; // skip malformed geometry, never fail the tile
            };
            match &layer.paint {
                PaintKind::Fill { color } => draw_fill(&mut pixmap, &geom, &proj, *color),
                PaintKind::Line { color, width } => draw_line(
                    &mut pixmap,
                    &geom,
                    &proj,
                    *color,
                    width.at(f32::from(coord.z)),
                ),
                PaintKind::Text {
                    field,
                    size_px,
                    color,
                } => {
                    if let (Some(text), Geometry::Point(p)) =
                        (attrs.get(field).and_then(|v| v.as_str()), &geom)
                    {
                        if !text.is_empty() {
                            let (x, y) = proj.px(p.x(), p.y());
                            labels.push((x, y, text.to_string(), *size_px, *color));
                        }
                    }
                }
            }
        }
    }

    // Done with the DB; release the (read-only) transaction.
    tx.rollback().await?;

    // Styles with only fill layers (very low zoom) never hit the line branch
    // above — composite before labels so the relief still shows.
    if !shade_applied {
        if let Some(s) = &shade {
            crate::hillshade::composite(&mut pixmap, s, hs.strength);
        }
    }

    draw_labels(&mut pixmap, &font, &labels, style.background);

    pixmap
        .encode_png()
        .map_err(|e| RasterError::Render(format!("png: {e}")))
}

/// One layer's rows inside the (padded) envelope: WKB in 3857 + jsonb attrs.
/// Runs on the caller's render transaction so the per-render `SET LOCAL`
/// statement-timeout applies.
async fn fetch_layer(
    conn: &mut sqlx::PgConnection,
    def: &BasemapLayer,
    env: (f64, f64, f64, f64),
    simplify_tol_m: f64,
) -> Result<Vec<(Vec<u8>, Json)>, RasterError> {
    let geom = &def.geom_column;
    // Clip every non-point geometry to the tile box (in the source SRID) BEFORE
    // simplify + transform — exactly as the MVT path does. Without this, a
    // low-zoom tile fetches + transforms + simplifies whole country-spanning
    // coastline/water polygons in full, which takes 60s+ and blows the statement
    // timeout (so the tile never renders or caches). `ST_ClipByBox2D` cookie-cuts
    // the geometry to the tile first, turning that into a sub-second render. The
    // `bounds` CTE computes the 25833 envelope once for both the clip and the
    // `&&` index filter. Points are never clipped (in-or-out by the `&&`).
    let clipped = format!("ST_ClipByBox2D(g.{geom}, (SELECT env FROM bounds))");
    let src_geom = match (def.simplify, def.kind != GeomKind::Point) {
        (_, false) => format!("g.{geom}"),
        (true, true) => format!("ST_SimplifyPreserveTopology({clipped}, {simplify_tol_m})"),
        (false, true) => clipped,
    };
    let attrs = if def.attrs.is_empty() {
        "'{}'::jsonb".to_string()
    } else {
        let kv: Vec<String> = def
            .attrs
            .iter()
            .map(|a| {
                let expr = a.expr.clone().unwrap_or_else(|| a.name.clone());
                format!("'{}', ({expr})", a.name)
            })
            .collect();
        format!("jsonb_build_object({})", kv.join(", "))
    };
    let extra = def
        .filter
        .as_ref()
        .map(|f| format!(" AND ({f})"))
        .unwrap_or_default();
    let sql = format!(
        "WITH bounds AS (\
           SELECT ST_Transform(ST_MakeEnvelope($1, $2, $3, $4, 3857), 25833) AS env\
         ) \
         SELECT ST_AsBinary(ST_Transform({src_geom}, 3857)) AS wkb, {attrs} AS attrs \
         FROM {table} g \
         WHERE g.{geom} && (SELECT env FROM bounds){extra}",
        table = def.table,
    );
    let rows: Vec<(Vec<u8>, Json)> = sqlx::query_as(&sql)
        .bind(env.0)
        .bind(env.1)
        .bind(env.2)
        .bind(env.3)
        .fetch_all(&mut *conn)
        .await?;
    Ok(rows)
}

fn sk_paint(color: Rgba) -> SkPaint<'static> {
    let mut p = SkPaint::default();
    p.set_color_rgba8(color.r, color.g, color.b, color.a);
    p.anti_alias = true;
    p
}

fn polygon_path(poly: &geo::Polygon<f64>, proj: &Proj, pb: &mut PathBuilder) {
    for (i, ring) in std::iter::once(poly.exterior())
        .chain(poly.interiors().iter())
        .enumerate()
    {
        let _ = i;
        let mut first = true;
        for c in ring.coords_iter_compat() {
            let (x, y) = proj.px(c.x, c.y);
            if first {
                pb.move_to(x, y);
                first = false;
            } else {
                pb.line_to(x, y);
            }
        }
        pb.close();
    }
}

/// Tiny compatibility shim: geo 0.28 exposes ring coords via `coords()`.
trait CoordsIterCompat {
    fn coords_iter_compat(&self) -> std::slice::Iter<'_, geo::Coord<f64>>;
}
impl CoordsIterCompat for geo::LineString<f64> {
    fn coords_iter_compat(&self) -> std::slice::Iter<'_, geo::Coord<f64>> {
        self.0.iter()
    }
}

pub(crate) fn draw_fill(pixmap: &mut Pixmap, geom: &Geometry<f64>, proj: &Proj, color: Rgba) {
    let mut pb = PathBuilder::new();
    match geom {
        Geometry::Polygon(p) => polygon_path(p, proj, &mut pb),
        Geometry::MultiPolygon(mp) => {
            for p in &mp.0 {
                polygon_path(p, proj, &mut pb);
            }
        }
        _ => return,
    }
    if let Some(path) = pb.finish() {
        pixmap.fill_path(
            &path,
            &sk_paint(color),
            FillRule::EvenOdd,
            Transform::identity(),
            None,
        );
    }
}

pub(crate) fn draw_line(
    pixmap: &mut Pixmap,
    geom: &Geometry<f64>,
    proj: &Proj,
    color: Rgba,
    width_px: f32,
) {
    let mut pb = PathBuilder::new();
    let mut add = |ls: &geo::LineString<f64>| {
        let mut first = true;
        for c in ls.coords_iter_compat() {
            let (x, y) = proj.px(c.x, c.y);
            if first {
                pb.move_to(x, y);
                first = false;
            } else {
                pb.line_to(x, y);
            }
        }
    };
    match geom {
        Geometry::LineString(ls) => add(ls),
        Geometry::MultiLineString(mls) => mls.0.iter().for_each(add),
        // Polygons can carry line paint (outlines) — stroke the rings.
        Geometry::Polygon(p) => {
            add(p.exterior());
            p.interiors().iter().for_each(add);
        }
        _ => return,
    }
    let Some(path) = pb.finish() else { return };
    let stroke = Stroke {
        width: width_px.max(0.2),
        line_cap: LineCap::Round,
        line_join: LineJoin::Round,
        ..Stroke::default()
    };
    pixmap.stroke_path(
        &path,
        &sk_paint(color),
        &stroke,
        Transform::identity(),
        None,
    );
}

/// Centered labels with a background-coloured halo and a naive keep-out so
/// overlapping names skip rather than stack.
fn draw_labels(
    pixmap: &mut Pixmap,
    font: &FontRef<'_>,
    labels: &[(f32, f32, String, f32, Rgba)],
    halo: Rgba,
) {
    let mut taken: Vec<(f32, f32, f32, f32)> = Vec::new();
    for (x, y, text, size, color) in labels {
        let scaled = font.as_scaled(*size * 1.2); // pt→px-ish fudge
        let w: f32 = text
            .chars()
            .map(|c| scaled.h_advance(scaled.scaled_glyph(c).id))
            .sum();
        let h = scaled.height();
        let (x0, y0) = (x - w / 2.0, y - h / 2.0);
        let rect = (x0, y0, x0 + w, y0 + h);
        if taken.iter().any(|t| overlaps(*t, rect)) {
            continue;
        }
        taken.push(rect);
        // Halo first (8 offsets), then the label itself.
        for (dx, dy) in [
            (-1.0, 0.0),
            (1.0, 0.0),
            (0.0, -1.0),
            (0.0, 1.0),
            (-1.0, -1.0),
            (1.0, 1.0),
            (-1.0, 1.0),
            (1.0, -1.0),
        ] {
            draw_text_run(pixmap, &scaled, text, x0 + dx, y0 + dy, halo);
        }
        draw_text_run(pixmap, &scaled, text, x0, y0, *color);
    }
}

fn overlaps(a: (f32, f32, f32, f32), b: (f32, f32, f32, f32)) -> bool {
    a.0 < b.2 && b.0 < a.2 && a.1 < b.3 && b.1 < a.3
}

fn draw_text_run(
    pixmap: &mut Pixmap,
    scaled: &ab_glyph::PxScaleFont<&FontRef<'_>>,
    text: &str,
    x0: f32,
    y0: f32,
    color: Rgba,
) {
    let mut pen_x = x0;
    let baseline = y0 + scaled.ascent();
    for ch in text.chars() {
        let mut glyph = scaled.scaled_glyph(ch);
        glyph.position = ab_glyph::point(pen_x, baseline);
        pen_x += scaled.h_advance(glyph.id);
        let Some(outline) = scaled.outline_glyph(glyph) else {
            continue;
        };
        let bounds = outline.px_bounds();
        let (w, h) = (pixmap.width() as i32, pixmap.height() as i32);
        outline.draw(|gx, gy, cov| {
            let px = bounds.min.x as i32 + gx as i32;
            let py = bounds.min.y as i32 + gy as i32;
            if px < 0 || py < 0 || px >= w || py >= h || cov <= 0.0 {
                return;
            }
            let idx = (py * w + px) as usize;
            let dst = &mut pixmap.pixels_mut()[idx];
            *dst = blend_over(*dst, color, cov.min(1.0));
        });
    }
}

/// Source-over blend of a straight-alpha colour at `cov` onto a
/// premultiplied destination pixel.
fn blend_over(dst: PremultipliedColorU8, src: Rgba, cov: f32) -> PremultipliedColorU8 {
    let sa = (f32::from(src.a) / 255.0) * cov;
    let blend = |s: u8, d: u8| -> u8 {
        ((f32::from(s) / 255.0 * sa + f32::from(d) / 255.0 * (1.0 - sa)) * 255.0).round() as u8
    };
    PremultipliedColorU8::from_rgba(
        blend(src.r, dst.red()),
        blend(src.g, dst.green()),
        blend(src.b, dst.blue()),
        ((sa + f32::from(dst.alpha()) / 255.0 * (1.0 - sa)) * 255.0).round() as u8,
    )
    .unwrap_or(dst)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::style::Width;

    fn px_at(pixmap: &Pixmap, x: u32, y: u32) -> (u8, u8, u8) {
        let p = pixmap.pixels()[(y * pixmap.width() + x) as usize];
        (p.red(), p.green(), p.blue())
    }

    #[test]
    fn tile_envelope_is_the_mercator_world_at_z0() {
        let env = tile_envelope_3857(TileCoord::new(0, 0, 0).unwrap());
        assert!((env.0 + WORLD_M).abs() < 1e-6);
        assert!((env.2 - WORLD_M).abs() < 1e-6);
        assert!((env.3 - WORLD_M).abs() < 1e-6);
    }

    #[test]
    fn fill_paints_polygon_interior_not_exterior() {
        let coord = TileCoord::new(0, 0, 0).unwrap();
        let env = tile_envelope_3857(coord);
        let proj = Proj::new(env, 256);
        let mut pixmap = Pixmap::new(256, 256).unwrap();
        pixmap.fill(tiny_skia::Color::WHITE);
        // A polygon covering the world's central quarter.
        let q = WORLD_M / 2.0;
        let poly = geo::Polygon::new(
            geo::LineString::from(vec![(-q, -q), (q, -q), (q, q), (-q, q), (-q, -q)]),
            vec![],
        );
        draw_fill(
            &mut pixmap,
            &Geometry::Polygon(poly),
            &proj,
            Rgba::rgb(185, 217, 241),
        );
        assert_eq!(px_at(&pixmap, 128, 128), (185, 217, 241), "center filled");
        assert_eq!(px_at(&pixmap, 10, 10), (255, 255, 255), "corner untouched");
    }

    #[test]
    fn line_strokes_at_requested_width() {
        let coord = TileCoord::new(0, 0, 0).unwrap();
        let proj = Proj::new(tile_envelope_3857(coord), 256);
        let mut pixmap = Pixmap::new(256, 256).unwrap();
        pixmap.fill(tiny_skia::Color::WHITE);
        let ls = geo::LineString::from(vec![(-WORLD_M, 0.0), (WORLD_M, 0.0)]);
        draw_line(
            &mut pixmap,
            &Geometry::LineString(ls),
            &proj,
            Rgba::rgb(160, 82, 45),
            4.0,
        );
        assert_eq!(px_at(&pixmap, 128, 128), (160, 82, 45), "on the line");
        assert_eq!(px_at(&pixmap, 128, 100), (255, 255, 255), "off the line");
    }

    #[test]
    fn width_lookup_is_exercised_by_the_house_style() {
        // Guard: Width::at is what the raster path uses at integer zooms.
        let w = Width::Stops(vec![(9.0, 0.8), (15.0, 3.0)]);
        assert!(w.at(12.0) > 0.8 && w.at(12.0) < 3.0);
    }
}
