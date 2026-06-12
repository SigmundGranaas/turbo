//! Multi-layer basemap MVT generation.
//!
//! Unlike [`crate::tile::render_tile`] — which emits one MVT layer for a
//! single curated resource — this stitches many feature classes (water,
//! landcover, contours, buildings, roads, place labels) into one vector
//! tile, the way a topo basemap is consumed. Layers are declared in
//! `tools/basemap-layers.toml` (config-only growth), so adding a class is a
//! TOML edit, not new code.
//!
//! ## How the bytes are assembled
//!
//! MVT is a protobuf `Tile` whose `layers` is a *repeated* field, so the
//! wire encoding of a two-layer tile is byte-identical to the concatenation
//! of two independently-encoded single-layer tiles. We exploit that: one SQL
//! statement runs `ST_AsMVT` once per active layer and concatenates the
//! `bytea` results with `||`. One round trip, no Rust-side protobuf work.
//!
//! ## Zoom handling
//!
//! Each layer declares `[min_zoom, max_zoom]`; out-of-range layers are
//! skipped entirely (their subquery is never emitted). Line/polygon layers
//! can opt into `simplify`, which applies `ST_SimplifyPreserveTopology` in
//! the metric 25833 space and drops sub-pixel polygons before projection —
//! keeping low-zoom tiles small.

use serde::Deserialize;
use turbo_tiles_core::tile::TileCoord;
use turbo_tiles_db::DbPool;

use crate::tile::MvtError;

/// Embedded fallback used when `tools/basemap-layers.toml` isn't on disk
/// (e.g. running from a different CWD or a stripped container image).
const EMBEDDED: &str = include_str!("../../../tools/basemap-layers.toml");

#[derive(Debug, Clone, Deserialize)]
pub struct BasemapConfig {
    #[serde(default)]
    pub layer: Vec<BasemapLayer>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GeomKind {
    Polygon,
    Line,
    Point,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AttrCol {
    /// MVT feature property name.
    pub name: String,
    /// SQL expression evaluated against the source row (defaults to `name`).
    #[serde(default)]
    pub expr: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BasemapLayer {
    /// MVT layer name the client/style references.
    pub name: String,
    /// Schema-qualified source table or view.
    pub table: String,
    #[serde(default = "default_geom")]
    pub geom_column: String,
    pub kind: GeomKind,
    pub min_zoom: u8,
    #[serde(default = "default_max_zoom")]
    pub max_zoom: u8,
    #[serde(default)]
    pub attrs: Vec<AttrCol>,
    /// Extra SQL predicate, ANDed into the WHERE (e.g. `deleted_at IS NULL`).
    #[serde(default)]
    pub filter: Option<String>,
    /// Apply zoom-scaled `ST_SimplifyPreserveTopology` + sub-pixel area drop.
    #[serde(default)]
    pub simplify: bool,
    /// Pre-generalised materialized view (schema-qualified) used instead of
    /// `table` at low zoom. It already exposes this layer's attrs and bakes
    /// in its filters, so the render skips simplify + filter when reading it.
    #[serde(default)]
    pub overview_table: Option<String>,
    /// Read `overview_table` when `z <= overview_max_zoom`. Ignored without
    /// `overview_table`.
    #[serde(default)]
    pub overview_max_zoom: Option<u8>,
}

impl BasemapLayer {
    /// The source table + whether the render must still simplify/filter,
    /// given the tile zoom. At low zoom this is the pre-generalised overview
    /// matview (no further work); otherwise the base table.
    fn source_at(&self, z: u8) -> (&str, bool, Option<&str>) {
        match (&self.overview_table, self.overview_max_zoom) {
            (Some(ov), Some(max)) if z <= max => (ov.as_str(), false, None),
            _ => (self.table.as_str(), self.simplify, self.filter.as_deref()),
        }
    }
}

fn default_geom() -> String {
    "geom".to_string()
}
fn default_max_zoom() -> u8 {
    22
}

impl BasemapConfig {
    /// Load from `tools/basemap-layers.toml` if present, else the embedded
    /// copy. Mirrors the cost-config / preset loader pattern.
    pub fn load_or_default() -> Self {
        let path = std::path::Path::new("tools/basemap-layers.toml");
        let text = std::fs::read_to_string(path)
            .ok()
            .unwrap_or_else(|| EMBEDDED.to_string());
        toml::from_str(&text).unwrap_or(BasemapConfig { layer: Vec::new() })
    }

    /// Layers active at `z`, in declaration order (= paint order).
    pub fn active(&self, z: u8) -> impl Iterator<Item = &BasemapLayer> {
        self.layer
            .iter()
            .filter(move |l| z >= l.min_zoom && z <= l.max_zoom)
    }
}

/// Web-mercator ground resolution (m/px) at the given zoom, taken at 60°N —
/// representative for Norway. Used to scale simplification + the sub-pixel
/// polygon drop so low-zoom tiles stay small.
fn ground_resolution_m(z: u8) -> f64 {
    // 156543.03 m/px at the equator (z0, 256px tiles) × cos(lat).
    156_543.033_928_041 * 0.5 / 2f64.powi(z as i32)
}

/// Render one multi-layer basemap MVT for `coord`. Empty when no active
/// layer has features in the envelope — clients treat that as a blank tile.
pub async fn render_basemap_tile(
    pool: &DbPool,
    config: &BasemapConfig,
    coord: TileCoord,
) -> Result<Vec<u8>, MvtError> {
    let active: Vec<&BasemapLayer> = config.active(coord.z).collect();
    if active.is_empty() {
        return Ok(Vec::new());
    }

    let res = ground_resolution_m(coord.z);
    // Half a pixel of simplification; drop polygons smaller than ~one pixel.
    let simplify_tol_m = res * 0.5;
    let min_area_m2 = res * res;

    // One CTE per active layer + a final concatenating SELECT.
    let mut ctes: Vec<String> = Vec::with_capacity(active.len());
    let mut concat_terms: Vec<String> = Vec::with_capacity(active.len());

    for (i, layer) in active.iter().enumerate() {
        let cte = format!("l{i}");
        let geom = &layer.geom_column;
        // Pick the base table or the pre-generalised low-zoom overview.
        let (table, simplify, filter) = layer.source_at(coord.z);

        // Optionally simplify in metric space before projecting to 3857.
        // The overview matview is already simplified, so `simplify` is false.
        let src_geom = if simplify && layer.kind != GeomKind::Point {
            format!("ST_SimplifyPreserveTopology(g.{geom}, {simplify_tol_m})")
        } else {
            format!("g.{geom}")
        };

        // Attribute projection: `expr AS "name"`, comma-separated.
        let attr_select = if layer.attrs.is_empty() {
            String::new()
        } else {
            let cols: Vec<String> = layer
                .attrs
                .iter()
                .map(|a| {
                    let expr = a.expr.clone().unwrap_or_else(|| a.name.clone());
                    format!("{expr} AS \"{}\"", a.name)
                })
                .collect();
            format!(", {}", cols.join(", "))
        };

        // WHERE: bbox hit + optional filter + sub-pixel drop for polygons.
        // Overview matviews bake in the filter + area drop, so both are gated
        // on the live source path (filter is None / simplify is false there).
        let mut wheres = vec![format!("g.{geom} && (SELECT env25833 FROM bounds)")];
        if let Some(f) = filter {
            wheres.push(format!("({f})"));
        }
        if simplify && layer.kind == GeomKind::Polygon {
            wheres.push(format!("ST_Area(g.{geom}) > {min_area_m2}"));
        }
        let where_clause = wheres.join(" AND ");

        ctes.push(format!(
            "{cte} AS (\n  \
               SELECT ST_AsMVT(t, '{name}', 4096, 'geom') AS mvt FROM (\n    \
                 SELECT ST_AsMVTGeom(ST_Transform({src_geom}, 3857), \
                          (SELECT env3857 FROM bounds), 4096, 64, true) AS geom{attr_select}\n    \
                 FROM {table} g\n    \
                 WHERE {where_clause}\n  \
               ) t WHERE t.geom IS NOT NULL\n)",
            name = layer.name,
        ));
        concat_terms.push(format!("COALESCE((SELECT mvt FROM {cte}), ''::bytea)"));
    }

    let sql = format!(
        "WITH bounds AS (\n  \
           SELECT ST_TileEnvelope($1::int, $2::int, $3::int) AS env3857,\n         \
                  ST_Transform(ST_TileEnvelope($1::int, $2::int, $3::int), 25833) AS env25833\n),\n{}\n\
         SELECT {}",
        ctes.join(",\n"),
        concat_terms.join(" || ")
    );

    let (bytes,): (Vec<u8>,) = sqlx::query_as(&sql)
        .bind(coord.z as i32)
        .bind(coord.x as i32)
        .bind(coord.y as i32)
        .fetch_one(pool)
        .await?;
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn overview_routing_picks_matview_at_low_zoom_only() {
        let layer = BasemapLayer {
            name: "water".into(),
            table: "terrain.water_polygon".into(),
            geom_column: "geom".into(),
            kind: GeomKind::Polygon,
            min_zoom: 4,
            max_zoom: 22,
            attrs: vec![],
            filter: Some("g.x".into()),
            simplify: true,
            overview_table: Some("basemap.water_overview".into()),
            overview_max_zoom: Some(9),
        };
        // In the overview band: matview, no simplify, no filter (baked in).
        assert_eq!(layer.source_at(6), ("basemap.water_overview", false, None));
        assert_eq!(layer.source_at(9), ("basemap.water_overview", false, None));
        // Above the band: base table, simplify on, filter applied.
        let (t, s, f) = layer.source_at(10);
        assert_eq!(t, "terrain.water_polygon");
        assert!(s);
        assert_eq!(f, Some("g.x"));
    }

    #[test]
    fn layer_without_overview_always_uses_base_table() {
        let cfg = BasemapConfig::load_or_default();
        let glacier = cfg.layer.iter().find(|l| l.name == "glacier").unwrap();
        assert!(glacier.overview_table.is_none());
        assert_eq!(glacier.source_at(4).0, "terrain.glacier_polygon");
    }

    #[test]
    fn house_config_overviews_are_wired() {
        let cfg = BasemapConfig::load_or_default();
        for (name, ov) in [
            ("water", "basemap.water_overview"),
            ("landcover", "basemap.landcover_overview"),
            ("coastline", "basemap.coastline_overview"),
            ("transportation", "basemap.transportation_overview"),
            ("contour", "basemap.contour_overview"),
        ] {
            let l = cfg.layer.iter().find(|l| l.name == name).unwrap();
            assert_eq!(l.overview_table.as_deref(), Some(ov), "{name} overview");
            assert!(l.overview_max_zoom.is_some(), "{name} overview_max_zoom");
        }
    }

    #[test]
    fn embedded_config_parses_and_has_layers() {
        let cfg: BasemapConfig = toml::from_str(EMBEDDED).expect("embedded toml parses");
        assert!(
            !cfg.layer.is_empty(),
            "embedded basemap config must define layers"
        );
        // Core topo layers must be present.
        for want in [
            "water",
            "landcover",
            "contour",
            "building",
            "transportation",
            "place",
        ] {
            assert!(
                cfg.layer.iter().any(|l| l.name == want),
                "missing basemap layer `{want}`"
            );
        }
    }

    #[test]
    fn zoom_gating_hides_detail_layers_at_low_zoom() {
        let cfg = BasemapConfig::load_or_default();
        let low: Vec<_> = cfg.active(5).map(|l| l.name.as_str()).collect();
        // Buildings (z14+) and contours (z11+) must not appear at z5.
        assert!(!low.contains(&"building"));
        assert!(!low.contains(&"contour"));
        // Water is a low-zoom layer and should be present.
        assert!(cfg.active(5).any(|l| l.name == "water"));
    }

    #[test]
    fn active_layers_are_in_declaration_paint_order() {
        let cfg = BasemapConfig::load_or_default();
        let names: Vec<_> = cfg.active(16).map(|l| l.name.as_str()).collect();
        // Fills before lines before labels: water before transportation before place.
        let wi = names.iter().position(|n| *n == "water").unwrap();
        let ti = names.iter().position(|n| *n == "transportation").unwrap();
        let pi = names.iter().position(|n| *n == "place").unwrap();
        assert!(
            wi < ti && ti < pi,
            "paint order must be fills → lines → labels: {names:?}"
        );
    }
}
