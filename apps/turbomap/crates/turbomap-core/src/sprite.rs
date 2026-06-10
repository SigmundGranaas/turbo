//! Built-in sprite atlas for POI icons and route shields.
//!
//! Real deployments load a sprite sheet (a packed PNG + JSON index) the
//! same way MapLibre does. To keep the renderer self-contained and its
//! tests deterministic with no binary assets, the built-in atlas is drawn
//! *procedurally* at startup into one RGBA texture. The lookup contract —
//! a sprite *name* resolves to an atlas rectangle the icon pipeline samples
//! — is identical to an asset-backed atlas, so swapping in real sprites
//! later is a drop-in change.
//!
//! Sprites are authored in sRGB with straight alpha; the icon pipeline
//! uploads the atlas as `Rgba8UnormSrgb` so sampling decodes to linear and
//! the framebuffer re-encodes — the same colour-management contract the
//! rest of the renderer follows.

use std::collections::HashMap;

/// Atlas dimensions. Small — a handful of 48²-ish sprites fit comfortably.
pub const SPRITE_ATLAS_W: u32 = 256;
pub const SPRITE_ATLAS_H: u32 = 128;

/// Where a named sprite lives in the atlas, in atlas pixels.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpriteInfo {
    pub atlas_x: u32,
    pub atlas_y: u32,
    pub width: u32,
    pub height: u32,
}

pub struct SpriteAtlas {
    /// `SPRITE_ATLAS_W * SPRITE_ATLAS_H` RGBA pixels (sRGB, straight alpha).
    rgba: Vec<u8>,
    sprites: HashMap<String, SpriteInfo>,
}

impl Default for SpriteAtlas {
    fn default() -> Self {
        Self::new()
    }
}

impl SpriteAtlas {
    /// Build the built-in atlas. The procedural set is intentionally small
    /// and generic — recognisable shapes that stand in for a real sprite
    /// sheet's POI glyphs and shield backgrounds.
    pub fn new() -> Self {
        let mut atlas = Self {
            rgba: vec![0; (SPRITE_ATLAS_W * SPRITE_ATLAS_H * 4) as usize],
            sprites: HashMap::new(),
        };
        // A POI dot: white disc with a red ring — the classic pin head.
        atlas.add_disc("dot", 0, 0, 28, [232, 64, 60], [255, 255, 255]);
        // A transit/stop dot in blue, to show data-driven selection works.
        atlas.add_disc("stop", 32, 0, 28, [40, 96, 210], [255, 255, 255]);
        // A route shield: rounded rect, white field, dark border — the
        // background a road `ref` number is centred on.
        atlas.add_shield("shield", (64, 0), (46, 30), [40, 54, 110], [255, 255, 255]);
        atlas
    }

    pub fn bitmap(&self) -> &[u8] {
        &self.rgba
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

    fn put(&mut self, x: u32, y: u32, rgba: [u8; 4]) {
        if x >= SPRITE_ATLAS_W || y >= SPRITE_ATLAS_H {
            return;
        }
        let i = ((y * SPRITE_ATLAS_W + x) * 4) as usize;
        self.rgba[i..i + 4].copy_from_slice(&rgba);
    }

    /// Filled disc with a coloured ring, antialiased at the rim.
    fn add_disc(&mut self, name: &str, ox: u32, oy: u32, d: u32, ring: [u8; 3], fill: [u8; 3]) {
        let r = d as f32 * 0.5;
        let cx = r - 0.5;
        let cy = r - 0.5;
        let ring_w = (d as f32 * 0.16).max(2.0);
        for gy in 0..d {
            for gx in 0..d {
                let dx = gx as f32 - cx;
                let dy = gy as f32 - cy;
                let dist = (dx * dx + dy * dy).sqrt();
                // Coverage falls off over one pixel at the outer rim.
                let outer = r - 0.5;
                let cov = (outer - dist + 0.5).clamp(0.0, 1.0);
                if cov <= 0.0 {
                    continue;
                }
                // Inside the ring band → ring colour; deeper in → fill.
                let rgb = if dist > outer - ring_w {
                    ring
                } else {
                    fill
                };
                let a = (cov * 255.0) as u8;
                self.put(ox + gx, oy + gy, [rgb[0], rgb[1], rgb[2], a]);
            }
        }
        self.sprites.insert(
            name.to_string(),
            SpriteInfo { atlas_x: ox, atlas_y: oy, width: d, height: d },
        );
    }

    /// Rounded-rectangle shield: a `border`-coloured frame around a `fill`
    /// field, with rounded corners.
    fn add_shield(
        &mut self,
        name: &str,
        (ox, oy): (u32, u32),
        (w, h): (u32, u32),
        border: [u8; 3],
        fill: [u8; 3],
    ) {
        let radius = (h as f32 * 0.32).max(3.0);
        let border_w = 2.0_f32;
        for gy in 0..h {
            for gx in 0..w {
                // Signed distance to the rounded-rect boundary (negative
                // inside). Standard rounded-box SDF.
                let px = gx as f32 + 0.5;
                let py = gy as f32 + 0.5;
                let qx = (px - w as f32 * 0.5).abs() - (w as f32 * 0.5 - radius);
                let qy = (py - h as f32 * 0.5).abs() - (h as f32 * 0.5 - radius);
                let outside = ((qx.max(0.0)).powi(2) + (qy.max(0.0)).powi(2)).sqrt()
                    + qx.max(qy).min(0.0)
                    - radius;
                let cov = (-outside + 0.5).clamp(0.0, 1.0);
                if cov <= 0.0 {
                    continue;
                }
                let rgb = if outside > -border_w { border } else { fill };
                let a = (cov * 255.0) as u8;
                self.put(ox + gx, oy + gy, [rgb[0], rgb[1], rgb[2], a]);
            }
        }
        self.sprites.insert(
            name.to_string(),
            SpriteInfo { atlas_x: ox, atlas_y: oy, width: w, height: h },
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn built_in_atlas_has_named_sprites() {
        let atlas = SpriteAtlas::new();
        assert!(atlas.get("dot").is_some());
        assert!(atlas.get("shield").is_some());
        assert!(atlas.get("stop").is_some());
        assert!(atlas.get("nonexistent").is_none());
    }

    #[test]
    fn sprite_rects_stay_inside_the_atlas() {
        let atlas = SpriteAtlas::new();
        for name in ["dot", "stop", "shield"] {
            let s = atlas.get(name).unwrap();
            assert!(s.atlas_x + s.width <= SPRITE_ATLAS_W, "{name} overflows width");
            assert!(s.atlas_y + s.height <= SPRITE_ATLAS_H, "{name} overflows height");
        }
    }

    #[test]
    fn disc_centre_is_opaque_and_corner_is_transparent() {
        let atlas = SpriteAtlas::new();
        let s = atlas.get("dot").unwrap();
        let at = |x: u32, y: u32| -> [u8; 4] {
            let i = (((s.atlas_y + y) * SPRITE_ATLAS_W + (s.atlas_x + x)) * 4) as usize;
            atlas.rgba[i..i + 4].try_into().unwrap()
        };
        // Centre is fully opaque, the corner of the bounding box is empty.
        assert_eq!(at(s.width / 2, s.height / 2)[3], 255);
        assert_eq!(at(0, 0)[3], 0);
    }
}
