//! Built-in SDF sprite atlas for POI icons and route shields.
//!
//! Real deployments load a sprite sheet (a packed PNG + JSON index) the same
//! way MapLibre does. To keep the renderer self-contained and its tests
//! deterministic with no binary assets, the built-in atlas is generated
//! *procedurally* at startup. The lookup contract — a sprite *name* resolves
//! to an atlas rectangle the icon pipeline samples — is identical to an
//! asset-backed atlas, so swapping in real sprites later is a drop-in change.
//!
//! Icons are stored as a **signed distance field** (single-channel R8, the
//! same 8SSEDT encoding as glyphs), not RGBA. That buys two things a raster
//! sprite can't: crisp edges at *any* on-screen size (the field is
//! resolution-independent), and per-feature **tinting** — the shape is
//! monochrome, coloured by the layer's `icon-color` at draw time. A filled
//! tintable rounded-rect under centred text is a route shield; a tintable
//! pin/disc is a POI marker.

use std::collections::HashMap;

use crate::text::generate_sdf;

/// Atlas dimensions. A handful of ~48² SDF sprites fit comfortably.
pub const SPRITE_ATLAS_W: u32 = 256;
pub const SPRITE_ATLAS_H: u32 = 128;

/// Pixels of SDF "outside" band around each icon, mirroring the glyph atlas.
/// The field falls off over this many native pixels, which the shader maps
/// to a one-screen-pixel antialiased edge at any scale.
const PAD: u32 = 6;

/// Where a named sprite lives in the atlas, in atlas pixels (the rect
/// *includes* the SDF padding band).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpriteInfo {
    pub atlas_x: u32,
    pub atlas_y: u32,
    pub width: u32,
    pub height: u32,
}

pub struct SpriteAtlas {
    /// `SPRITE_ATLAS_W * SPRITE_ATLAS_H` single-channel SDF bytes (128 = on
    /// the contour, lower inside, higher outside).
    sdf: Vec<u8>,
    sprites: HashMap<String, SpriteInfo>,
}

impl Default for SpriteAtlas {
    fn default() -> Self {
        Self::new()
    }
}

impl SpriteAtlas {
    /// Build the built-in atlas. The procedural set is intentionally small
    /// and generic — recognisable monochrome shapes that stand in for a real
    /// sprite sheet's POI glyphs and shield backgrounds.
    pub fn new() -> Self {
        let mut atlas = Self {
            sdf: vec![128; (SPRITE_ATLAS_W * SPRITE_ATLAS_H) as usize],
            sprites: HashMap::new(),
        };
        // A POI dot (filled disc) and a rounded "stop" square.
        atlas.add_sdf("dot", 0, 0, 36, 36, |x, y, w, h| {
            let (cx, cy) = (w * 0.5, h * 0.5);
            let r = w.min(h) * 0.5 - 0.5;
            (x - cx).hypot(y - cy) <= r
        });
        atlas.add_sdf("stop", 52, 0, 36, 36, |x, y, w, h| {
            rounded_box(x, y, w, h, w * 0.22) <= 0.0
        });
        // A classic teardrop pin: a head circle tapering to a point.
        atlas.add_sdf("marker", 104, 0, 34, 46, |x, y, w, h| {
            let cx = w * 0.5;
            let head_r = w * 0.42;
            let head_cy = head_r + 1.0;
            if (x - cx).hypot(y - head_cy) <= head_r {
                return true;
            }
            // Cone narrowing from the head's centre line down to the tip.
            if y >= head_cy && y <= h {
                let t = (y - head_cy) / (h - head_cy);
                let half = head_r * (1.0 - t);
                return (x - cx).abs() <= half;
            }
            false
        });
        // A route shield: a filled rounded rectangle, tinted, with a road
        // ref centred on top by the text pass.
        atlas.add_sdf("shield", 152, 0, 48, 30, |x, y, w, h| {
            rounded_box(x, y, w, h, h * 0.32) <= 0.0
        });
        // POI category markers (second atlas row, y≥60). Bold geometric
        // shapes that stay legible at ~12 px where pictographs blur.
        // A plus/cross — health & services.
        atlas.add_sdf("cross", 0, 60, 28, 28, |x, y, w, h| {
            let (t, ins) = (0.22, 0.08);
            let hbar = (y - h * 0.5).abs() <= h * t && x >= w * ins && x <= w * (1.0 - ins);
            let vbar = (x - w * 0.5).abs() <= w * t && y >= h * ins && y <= h * (1.0 - ins);
            hbar || vbar
        });
        // A diamond — culture, lodging & landmarks.
        atlas.add_sdf("diamond", 44, 60, 28, 28, |x, y, w, h| {
            (x - w * 0.5).abs() / (w * 0.46) + (y - h * 0.5).abs() / (h * 0.46) <= 1.0
        });
        // A fork — food & drink.
        atlas.add_sdf("fork", 88, 60, 24, 30, |x, y, w, h| {
            let cx = w * 0.5;
            let stem = (x - cx).abs() <= w * 0.13 && (h * 0.42..=h * 0.95).contains(&y);
            let neck = (h * 0.38..=h * 0.50).contains(&y) && (w * 0.26..=w * 0.74).contains(&x);
            let tine = |px: f32| (x - px).abs() <= w * 0.11 && (h * 0.06..=h * 0.45).contains(&y);
            stem || neck || tine(w * 0.30) || tine(w * 0.5) || tine(w * 0.70)
        });
        atlas
    }

    /// Single-channel SDF bytes, row-major, `SPRITE_ATLAS_W` wide.
    pub fn bitmap(&self) -> &[u8] {
        &self.sdf
    }

    pub fn get(&self, name: &str) -> Option<SpriteInfo> {
        self.sprites.get(name).copied()
    }

    /// Number of named sprites — used by tests.
    pub fn len(&self) -> usize {
        self.sprites.len()
    }

    pub fn is_empty(&self) -> bool {
        self.sprites.is_empty()
    }

    /// Rasterise a monochrome shape (`inside` tests native shape coords in
    /// `[0,w)×[0,h)`), convert it to an SDF, and blit it into the atlas at
    /// `(ox, oy)`. The stored rect includes the `PAD` band on every side.
    fn add_sdf<F: Fn(f32, f32, f32, f32) -> bool>(
        &mut self,
        name: &str,
        ox: u32,
        oy: u32,
        nat_w: u32,
        nat_h: u32,
        inside: F,
    ) {
        let pw = nat_w + 2 * PAD;
        let ph = nat_h + 2 * PAD;
        let mut mask = vec![0u8; (pw * ph) as usize];
        for y in 0..ph {
            for x in 0..pw {
                let nx = x as f32 - PAD as f32 + 0.5;
                let ny = y as f32 - PAD as f32 + 0.5;
                if inside(nx, ny, nat_w as f32, nat_h as f32) {
                    mask[(y * pw + x) as usize] = 255;
                }
            }
        }
        let sdf = generate_sdf(&mask, pw, ph, PAD as f32);
        for y in 0..ph {
            for x in 0..pw {
                let ax = ox + x;
                let ay = oy + y;
                if ax < SPRITE_ATLAS_W && ay < SPRITE_ATLAS_H {
                    self.sdf[(ay * SPRITE_ATLAS_W + ax) as usize] = sdf[(y * pw + x) as usize];
                }
            }
        }
        self.sprites.insert(
            name.to_string(),
            SpriteInfo { atlas_x: ox, atlas_y: oy, width: pw, height: ph },
        );
    }
}

/// Signed distance to a rounded rectangle centred in `[0,w)×[0,h)` with
/// corner `radius`. Negative inside. Standard rounded-box SDF.
fn rounded_box(x: f32, y: f32, w: f32, h: f32, radius: f32) -> f32 {
    let qx = (x - w * 0.5).abs() - (w * 0.5 - radius);
    let qy = (y - h * 0.5).abs() - (h * 0.5 - radius);
    (qx.max(0.0)).hypot(qy.max(0.0)) + qx.max(qy).min(0.0) - radius
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn built_in_atlas_has_named_sprites() {
        let atlas = SpriteAtlas::new();
        for name in ["dot", "stop", "marker", "shield", "cross", "diamond", "fork"] {
            assert!(atlas.get(name).is_some(), "missing {name}");
        }
        assert!(atlas.get("nonexistent").is_none());
    }

    #[test]
    fn sprite_rects_stay_inside_the_atlas() {
        let atlas = SpriteAtlas::new();
        for name in ["dot", "stop", "marker", "shield", "cross", "diamond", "fork"] {
            let s = atlas.get(name).unwrap();
            assert!(s.atlas_x + s.width <= SPRITE_ATLAS_W, "{name} overflows width");
            assert!(s.atlas_y + s.height <= SPRITE_ATLAS_H, "{name} overflows height");
        }
    }

    #[test]
    fn sdf_is_inside_at_centre_and_outside_at_corner() {
        // SDF convention (shared with glyphs): 128 = contour, < 128 inside,
        // > 128 outside. A filled disc must read inside at its centre and
        // outside at the padded corner.
        let atlas = SpriteAtlas::new();
        let s = atlas.get("dot").unwrap();
        let at = |x: u32, y: u32| -> u8 {
            atlas.sdf[((s.atlas_y + y) * SPRITE_ATLAS_W + (s.atlas_x + x)) as usize]
        };
        assert!(at(s.width / 2, s.height / 2) < 128, "centre should be inside");
        assert!(at(0, 0) > 128, "corner should be outside");
    }
}
