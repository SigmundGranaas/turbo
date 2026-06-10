//! Server-side sprite sheet for MapLibre/Mapbox GL icon layers.
//!
//! MapLibre fetches `{sprite}.json` (an index) + `{sprite}.png` (the atlas),
//! plus `@2x` variants for retina. The basemap style's `place` layers use
//! `icon-image: {kind}`, so the sprite keys match the `anchors.anchor.kind`
//! values (summit / cabin / waterfeature / named_place). Icons are tiny
//! topo glyphs drawn with tiny-skia, generated on demand and cached by the
//! edge worker (its allowlist already covers `/sprite*`).

use std::fmt::Write as _;

use tiny_skia::{FillRule, Paint, PathBuilder, Pixmap, Transform};

/// Logical icon box at 1×, in px. Icons are drawn centred with a 1px margin.
const ICON: u32 = 15;

/// The icons, in atlas order. Keys match `place` `kind` values.
const ICONS: &[&str] = &["summit", "cabin", "waterfeature", "named_place"];

/// Build the sprite atlas PNG + JSON index at the given pixel ratio (1 or 2).
/// Returns `(png_bytes, json_string)`.
pub fn build(ratio: u32) -> Result<(Vec<u8>, String), SpriteError> {
    let ratio = ratio.max(1);
    let s = ICON * ratio;
    let n = ICONS.len() as u32;
    let mut atlas = Pixmap::new(s * n, s).ok_or(SpriteError("atlas alloc"))?;

    let mut json = String::from("{");
    for (i, name) in ICONS.iter().enumerate() {
        let x0 = i as u32 * s;
        draw_icon(&mut atlas, name, x0, s);
        if i > 0 {
            json.push(',');
        }
        let _ = write!(
            json,
            "\"{name}\":{{\"x\":{x0},\"y\":0,\"width\":{s},\"height\":{s},\"pixelRatio\":{ratio},\"sdf\":false}}"
        );
    }
    json.push('}');

    let png = atlas.encode_png().map_err(|_| SpriteError("png encode"))?;
    Ok((png, json))
}

fn paint(r: u8, g: u8, b: u8) -> Paint<'static> {
    let mut p = Paint::default();
    p.set_color_rgba8(r, g, b, 255);
    p.anti_alias = true;
    p
}

fn fill(pix: &mut Pixmap, pb: PathBuilder, p: &Paint) {
    if let Some(path) = pb.finish() {
        pix.fill_path(&path, p, FillRule::Winding, Transform::identity(), None);
    }
}

/// Draw one icon into the atlas at horizontal offset `x0`, box size `s`.
fn draw_icon(pix: &mut Pixmap, name: &str, x0: u32, s: u32) {
    let x0 = x0 as f32;
    let s = s as f32;
    let m = s * 0.12; // margin
    let (lo, hi) = (x0 + m, x0 + s - m);
    let (top, bot) = (m, s - m);
    let cx = x0 + s / 2.0;
    let cy = s / 2.0;

    match name {
        // Summit: a brown peak triangle.
        "summit" => {
            let mut pb = PathBuilder::new();
            pb.move_to(cx, top);
            pb.line_to(hi, bot);
            pb.line_to(lo, bot);
            pb.close();
            fill(pix, pb, &paint(0x8a, 0x5a, 0x2a));
        }
        // Cabin: a dark hut — square body + roof.
        "cabin" => {
            let mut roof = PathBuilder::new();
            roof.move_to(lo, cy);
            roof.line_to(cx, top);
            roof.line_to(hi, cy);
            roof.close();
            fill(pix, roof, &paint(0x5a, 0x3a, 0x22));
            let mut body = PathBuilder::new();
            body.push_rect(tiny_skia::Rect::from_ltrb(lo, cy, hi, bot).unwrap());
            fill(pix, body, &paint(0x6b, 0x4a, 0x2e));
        }
        // Water feature: a blue disc.
        "waterfeature" => {
            let mut pb = PathBuilder::new();
            pb.push_circle(cx, cy, s * 0.30);
            fill(pix, pb, &paint(0x3a, 0x7b, 0xb5));
        }
        // Generic named place: a small grey dot.
        _ => {
            let mut pb = PathBuilder::new();
            pb.push_circle(cx, cy, s * 0.16);
            fill(pix, pb, &paint(0x55, 0x55, 0x55));
        }
    }
}

#[derive(Debug, thiserror::Error)]
#[error("sprite: {0}")]
pub struct SpriteError(&'static str);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sprite_json_indexes_every_icon_at_both_ratios() {
        for ratio in [1u32, 2] {
            let (png, json) = build(ratio).expect("build");
            assert!(!png.is_empty());
            let v: serde_json::Value = serde_json::from_str(&json).unwrap();
            for name in ICONS {
                let e = &v[name];
                assert_eq!(e["pixelRatio"], ratio, "{name}@{ratio}x ratio");
                assert_eq!(e["width"], ICON * ratio, "{name}@{ratio}x width");
                assert_eq!(e["height"], ICON * ratio);
                assert_eq!(e["sdf"], false);
            }
            // Atlas is a row of n icons.
            let w: u64 = (ICON * ratio * ICONS.len() as u32) as u64;
            // x of the last icon = (n-1)*s
            assert_eq!(v["named_place"]["x"], (ICON * ratio * 3) as u64);
            assert!(w > 0);
        }
    }

    #[test]
    fn png_decodes_to_the_expected_atlas_size() {
        let (png, _) = build(2).unwrap();
        let pm = Pixmap::decode_png(&png).unwrap();
        assert_eq!(pm.width(), ICON * 2 * ICONS.len() as u32);
        assert_eq!(pm.height(), ICON * 2);
    }
}
