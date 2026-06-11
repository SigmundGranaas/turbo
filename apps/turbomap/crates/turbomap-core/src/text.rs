//! Glyph atlas + text layout + AABB collision.
//!
//! Strategy: rasterise glyphs lazily at a fixed pixel size into a single
//! greyscale-alpha atlas (shelf-packed). Lay out a string into screen-space
//! glyph quads, scaling from the raster size to the requested font size on
//! the GPU side. Reject any label whose AABB overlaps an already-placed
//! label.
//!
//! The atlas is keyed by `(face, glyph id)`, fed by a [`FontStack`] that
//! resolves per-character coverage across a default face (bundled Roboto)
//! plus host-supplied fallback faces (CJK, Arabic, …). [`FontAtlas::shape`]
//! runs the Unicode bidi algorithm + HarfBuzz (`rustybuzz`) so ligatures,
//! kerning, mark positioning, Arabic joining, Indic reordering and mixed
//! LTR/RTL ordering are all correct — it returns glyphs in visual order,
//! which the layout functions place by advance.

use std::collections::HashMap;

use ab_glyph::{Font, FontArc, GlyphId, PxScale, ScaleFont};

/// Bundled default font — Roboto Regular, Apache 2.0. See
/// `assets/LICENSE-fonts.md`. Covers Latin; hosts append fallback faces for
/// other scripts (CJK, Arabic, …) via [`FontAtlas::add_fallback_face`].
const FONT_BYTES: &[u8] = include_bytes!("../assets/Roboto-Regular.ttf");

/// Atlas side in pixels. 1024² × 1 byte = 1 MiB host-side bitmap, fits an
/// abundance of glyphs at the raster size below.
pub const ATLAS_SIZE: u32 = 1024;

/// All glyphs are rasterised at this pixel size; rendering scales them via
/// the GPU. Smaller = blurrier text at large sizes; larger = atlas fills
/// faster.
pub const RASTER_PX: f32 = 36.0;

/// Pixels of padding around each glyph in the atlas, used to hold the
/// signed-distance field's "outside" band. The fragment shader can render
/// a halo up to roughly this many raster pixels wide.
pub const SDF_PAD: u32 = 4;

/// Atlas SDF center is 128 (signed distance 0). Each unit-of-u8 corresponds
/// to roughly 1/8 of a raster pixel; the band of meaningful values is
/// [128 - SDF_PAD*32, 128 + SDF_PAD*32]. Anything outside is clamped.
pub const SDF_CENTER: u8 = 128;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GlyphInfo {
    /// Top-left corner of the glyph bitmap in atlas pixels.
    pub atlas_x: u32,
    pub atlas_y: u32,
    /// Glyph bitmap size in atlas pixels.
    pub width: u32,
    pub height: u32,
    /// Offset from the cursor to the bitmap top-left, in raster pixels.
    pub bearing_x: f32,
    pub bearing_y: f32,
    /// Cursor advance after this glyph, in raster pixels.
    pub advance: f32,
}

/// One shaped glyph in visual (left-to-right) order: which face + glyph to
/// raster, and the position HarfBuzz computed (advance + offset, in raster
/// pixels). Offsets are nonzero for combining marks (Arabic, Devanagari).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ShapedGlyph {
    pub face_id: usize,
    pub glyph_id: GlyphId,
    pub x_advance: f32,
    pub x_offset: f32,
    pub y_offset: f32,
}

/// A single loaded face: `ab_glyph` for outline rasterisation (by glyph id),
/// the raw bytes for `rustybuzz` shaping, and units-per-em to scale shaped
/// metrics into raster pixels.
struct Face {
    ab: FontArc,
    data: std::sync::Arc<Vec<u8>>,
    upem: f32,
}

/// An ordered set of font faces. Face 0 is the bundled default (Roboto);
/// hosts append fallback faces (CJK, Arabic, …) covering scripts the
/// default doesn't. Per-character coverage is resolved first-face-wins.
struct FontStack {
    faces: Vec<Face>,
}

impl FontStack {
    fn new() -> Self {
        let mut s = Self { faces: Vec::new() };
        s.add(FONT_BYTES.to_vec());
        s
    }

    /// Append a face from owned font bytes. Returns `false` if the bytes
    /// don't parse as a font (the face is simply not added).
    fn add(&mut self, data: Vec<u8>) -> bool {
        let Ok(ab) = FontArc::try_from_vec(data.clone()) else {
            return false;
        };
        // `rustybuzz` reads units-per-em (and validates the tables it needs).
        let Some(upem) = rustybuzz::Face::from_slice(&data, 0).map(|f| f.units_per_em()) else {
            return false;
        };
        self.faces.push(Face {
            ab,
            data: std::sync::Arc::new(data),
            upem: upem as f32,
        });
        true
    }

    /// Index of the first face whose cmap covers `c`; falls back to face 0
    /// (which renders `.notdef`) when no face covers it.
    fn face_for(&self, c: char) -> usize {
        self.faces
            .iter()
            .position(|f| f.ab.glyph_id(c).0 != 0)
            .unwrap_or(0)
    }

    fn ab(&self, id: usize) -> &FontArc {
        &self.faces[id].ab
    }

    fn len(&self) -> usize {
        self.faces.len()
    }

    /// Split `text` into contiguous `(face, substring)` groups in logical
    /// order, each group covered by a single face.
    fn itemize(&self, text: &str) -> Vec<(usize, String)> {
        let mut groups: Vec<(usize, String)> = Vec::new();
        for c in text.chars() {
            let fid = self.face_for(c);
            match groups.last_mut() {
                Some((f, s)) if *f == fid => s.push(c),
                _ => groups.push((fid, c.to_string())),
            }
        }
        groups
    }

    /// Shape one same-face substring with HarfBuzz, returning glyphs in
    /// visual order with metrics in raster pixels.
    fn shape_run(&self, face_id: usize, text: &str, rtl: bool) -> Vec<ShapedGlyph> {
        let face = &self.faces[face_id];
        let Some(rb) = rustybuzz::Face::from_slice(&face.data, 0) else {
            return Vec::new();
        };
        let mut buffer = rustybuzz::UnicodeBuffer::new();
        buffer.push_str(text);
        buffer.set_direction(if rtl {
            rustybuzz::Direction::RightToLeft
        } else {
            rustybuzz::Direction::LeftToRight
        });
        let glyphs = rustybuzz::shape(&rb, &[], buffer);
        let s = RASTER_PX / face.upem;
        glyphs
            .glyph_infos()
            .iter()
            .zip(glyphs.glyph_positions())
            .map(|(info, pos)| ShapedGlyph {
                face_id,
                glyph_id: GlyphId(info.glyph_id as u16),
                x_advance: pos.x_advance as f32 * s,
                x_offset: pos.x_offset as f32 * s,
                y_offset: pos.y_offset as f32 * s,
            })
            .collect()
    }
}

pub struct FontAtlas {
    stack: FontStack,
    /// `ATLAS_SIZE * ATLAS_SIZE` greyscale-alpha pixels.
    bitmap: Vec<u8>,
    /// Keyed by `(face index, glyph id)` so the same glyph id in different
    /// faces (and fallbacks) never collide.
    glyphs: HashMap<(usize, u16), GlyphInfo>,
    cursor_x: u32,
    cursor_y: u32,
    row_height: u32,
    dirty: bool,
}

impl Default for FontAtlas {
    fn default() -> Self {
        Self::new()
    }
}

impl FontAtlas {
    pub fn new() -> Self {
        Self {
            stack: FontStack::new(),
            // Clear to the "fully outside the glyph" SDF value (255), NOT 0.
            // In this field 0 = deep *inside* a glyph, so a zeroed atlas
            // makes every glyph cell's edge ramp 255→0 under bilinear
            // sampling, crossing the fill+halo thresholds and painting a
            // rectangular box around each glyph. 255 in the gutter matches
            // the cell-border texels (also ~255), so there is no ramp and
            // no box. See the `atlas_gutter_is_outside_value` test.
            bitmap: vec![255; (ATLAS_SIZE * ATLAS_SIZE) as usize],
            glyphs: HashMap::new(),
            cursor_x: 1,
            cursor_y: 1,
            row_height: 0,
            dirty: true,
        }
    }

    pub fn bitmap(&self) -> &[u8] {
        &self.bitmap
    }

    /// Append a fallback font face (e.g. a CJK or Arabic face supplied by
    /// the host). Returns `false` if the bytes don't parse. Faces added
    /// later have lower priority — face 0 (Roboto) always wins where it has
    /// coverage.
    pub fn add_fallback_face(&mut self, data: Vec<u8>) -> bool {
        self.stack.add(data)
    }

    /// Number of loaded faces (default + fallbacks).
    pub fn face_count(&self) -> usize {
        self.stack.len()
    }

    /// Read & clear the dirty flag — used by the GPU upload to know whether
    /// the atlas texture needs re-uploading this frame.
    pub fn take_dirty(&mut self) -> bool {
        let d = self.dirty;
        self.dirty = false;
        d
    }

    /// Shape `text` into glyphs in **visual** (left-to-right) order, ready to
    /// lay out by advance. Runs the Unicode bidi algorithm to order mixed
    /// LTR/RTL runs, picks a covering face per script (font fallback), and
    /// shapes each run with HarfBuzz (`rustybuzz`) so ligatures, kerning,
    /// mark positioning, Arabic joining and Indic reordering are correct.
    pub fn shape(&self, text: &str) -> Vec<ShapedGlyph> {
        if text.is_empty() {
            return Vec::new();
        }
        let bidi = unicode_bidi::BidiInfo::new(text, None);
        let mut out = Vec::new();
        for para in &bidi.paragraphs {
            // `visual_runs` returns the line's runs already in left-to-right
            // visual order; each run has a single embedding level.
            let (levels, runs) = bidi.visual_runs(para, para.range.clone());
            for run in runs {
                let rtl = levels[run.start].is_rtl();
                // A run may still mix scripts needing different faces. Split
                // by face in logical order; for an RTL run emit the groups
                // in reverse so the whole run reads left-to-right.
                let groups = self.stack.itemize(&text[run.clone()]);
                let ordered: Vec<(usize, String)> = if rtl {
                    groups.into_iter().rev().collect()
                } else {
                    groups
                };
                for (face_id, seg) in ordered {
                    out.extend(self.stack.shape_run(face_id, &seg, rtl));
                }
            }
        }
        out
    }

    /// Return the glyph info for `(face, glyph)`, rasterising and packing it
    /// on first sight. Returns `None` only if the atlas is full and the
    /// glyph genuinely cannot be added.
    pub fn ensure(&mut self, face_id: usize, glyph_id: GlyphId) -> Option<GlyphInfo> {
        let key = (face_id, glyph_id.0);
        if let Some(g) = self.glyphs.get(&key) {
            return Some(*g);
        }
        self.insert(face_id, glyph_id)
    }

    fn insert(&mut self, face_id: usize, glyph_id: GlyphId) -> Option<GlyphInfo> {
        let scale = PxScale::from(RASTER_PX);
        // Pull everything we need off the face first, then drop the borrow
        // so we can mutate the atlas bitmap below.
        let (advance, outlined) = {
            let face = self.stack.ab(face_id);
            let advance = face.as_scaled(scale).h_advance(glyph_id);
            let outlined = face.outline_glyph(glyph_id.with_scale(scale));
            (advance, outlined)
        };
        let (tight_w, tight_h, tight_bearing_x, tight_bearing_y) = match outlined.as_ref() {
            Some(o) => {
                let b = o.px_bounds();
                (
                    b.width().ceil() as u32,
                    b.height().ceil() as u32,
                    b.min.x,
                    b.min.y,
                )
            }
            // Invisible glyphs (space etc.) still need an advance recorded.
            None => (0, 0, 0.0, 0.0),
        };

        // Pad the bitmap by SDF_PAD on each side so the signed distance
        // field has room to fall off into the "outside" band. Bearing is
        // shifted by -SDF_PAD so the visible glyph stays in the same place
        // when rendered.
        let (width, height, bearing_x, bearing_y) = if tight_w > 0 && tight_h > 0 {
            (
                tight_w + 2 * SDF_PAD,
                tight_h + 2 * SDF_PAD,
                tight_bearing_x - SDF_PAD as f32,
                tight_bearing_y - SDF_PAD as f32,
            )
        } else {
            (0, 0, 0.0, 0.0)
        };

        // Shelf packer.
        if self.cursor_x + width + 1 > ATLAS_SIZE {
            self.cursor_y += self.row_height + 1;
            self.cursor_x = 1;
            self.row_height = 0;
        }
        if self.cursor_y + height + 1 > ATLAS_SIZE {
            return None;
        }

        let info = GlyphInfo {
            atlas_x: self.cursor_x,
            atlas_y: self.cursor_y,
            width,
            height,
            bearing_x,
            bearing_y,
            advance,
        };

        if let Some(o) = outlined {
            // Step 1: rasterise the alpha mask into a tight scratch buffer.
            let stride = width as usize;
            let mut alpha = vec![0u8; (width * height) as usize];
            o.draw(|gx, gy, coverage| {
                // Offset by SDF_PAD so the glyph sits inside the padded
                // region with `SDF_PAD` empty pixels around it.
                let bx = SDF_PAD as usize + gx as usize;
                let by = SDF_PAD as usize + gy as usize;
                let idx = by * stride + bx;
                if idx < alpha.len() {
                    alpha[idx] = (coverage * 255.0).clamp(0.0, 255.0) as u8;
                }
            });

            // Step 2: convert alpha → SDF.
            let sdf = generate_sdf(&alpha, width, height, SDF_PAD as f32);

            // Step 3: blit SDF into the atlas bitmap at (cursor_x, cursor_y).
            let ax = self.cursor_x as usize;
            let ay = self.cursor_y as usize;
            for y in 0..height as usize {
                for x in 0..width as usize {
                    let atlas_idx = (ay + y) * ATLAS_SIZE as usize + (ax + x);
                    if atlas_idx < self.bitmap.len() {
                        self.bitmap[atlas_idx] = sdf[y * stride + x];
                    }
                }
            }
        }

        self.glyphs.insert((face_id, glyph_id.0), info);
        self.cursor_x += width.max(1) + 1;
        self.row_height = self.row_height.max(height);
        self.dirty = true;
        Some(info)
    }

    pub fn ascent_at(font_size_px: f32) -> f32 {
        // Roboto's ascent ≈ 1888/2048 em, scaled by px size.
        font_size_px * (1888.0 / 2048.0)
    }
}

/// One positioned glyph ready to draw. All values are in pixels (screen
/// space).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LayoutGlyph {
    /// Top-left of the glyph quad in screen pixels.
    pub screen_x: f32,
    pub screen_y: f32,
    /// Quad size in screen pixels.
    pub width: f32,
    pub height: f32,
    /// Atlas-pixel coordinates (top-left + size). The renderer normalises
    /// these by `ATLAS_SIZE` when sampling.
    pub atlas_x: f32,
    pub atlas_y: f32,
    pub atlas_w: f32,
    pub atlas_h: f32,
}

/// One glyph placed *along a path*: an axis-aligned quad (`screen_*`,
/// `atlas_*` as in [`LayoutGlyph`]) plus the rotation that orients it to
/// the local path tangent. The renderer rotates the quad about `pivot`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PathGlyph {
    pub screen_x: f32,
    pub screen_y: f32,
    pub width: f32,
    pub height: f32,
    pub atlas_x: f32,
    pub atlas_y: f32,
    pub atlas_w: f32,
    pub atlas_h: f32,
    /// Screen-space point the quad rotates about (the glyph's pen point).
    pub pivot: (f32, f32),
    /// Rotation in radians (the path tangent at the pen point).
    pub angle: f32,
}

/// Lay `text` out following a screen-space polyline `path`, centred on the
/// line's arc length, each glyph rotated to the local tangent. Returns
/// `None` when the text is longer than the line (it can't fit) or the path
/// is degenerate — the caller drops the label rather than overflow it.
pub fn layout_along_path(
    text: &str,
    font_size_px: f32,
    path: &[(f32, f32)],
    atlas: &mut FontAtlas,
) -> Option<Vec<PathGlyph>> {
    if path.len() < 2 {
        return None;
    }
    // Read the path left-to-right: reverse a net-westbound line so glyphs
    // never come out upside down.
    let mut pts: Vec<(f32, f32)> = path.to_vec();
    if pts.last().unwrap().0 < pts.first().unwrap().0 {
        pts.reverse();
    }
    // Cumulative arc length.
    let mut cum = Vec::with_capacity(pts.len());
    cum.push(0.0_f32);
    for w in pts.windows(2) {
        let d = ((w[1].0 - w[0].0).powi(2) + (w[1].1 - w[0].1).powi(2)).sqrt();
        cum.push(cum.last().unwrap() + d);
    }
    let total = *cum.last().unwrap();
    if total <= 1.0 {
        return None;
    }

    let scale = font_size_px / RASTER_PX;
    let glyphs = atlas.shape(text);
    let text_width: f32 = glyphs.iter().map(|g| g.x_advance * scale).sum();
    if text_width > total {
        return None; // doesn't fit on this line
    }

    let mut pen = (total - text_width) * 0.5; // centre the run on the line
    let mut out = Vec::with_capacity(glyphs.len());
    for sg in &glyphs {
        let advance = sg.x_advance * scale;
        if let Some(g) = atlas.ensure(sg.face_id, sg.glyph_id) {
            if g.width > 0 && g.height > 0 {
                let (pivot, angle) = point_and_angle_at(&pts, &cum, pen);
                out.push(PathGlyph {
                    // Flat quad anchored at the pen point; the renderer
                    // rotates it about `pivot` (= the pen point) by `angle`.
                    screen_x: pivot.0 + (sg.x_offset + g.bearing_x) * scale,
                    screen_y: pivot.1 + (g.bearing_y - sg.y_offset) * scale,
                    width: g.width as f32 * scale,
                    height: g.height as f32 * scale,
                    atlas_x: g.atlas_x as f32,
                    atlas_y: g.atlas_y as f32,
                    atlas_w: g.width as f32,
                    atlas_h: g.height as f32,
                    pivot,
                    angle,
                });
            }
        }
        pen += advance;
    }
    if out.is_empty() {
        return None;
    }
    Some(out)
}

/// Sample a polyline at arc-length `dist`: the interpolated point and the
/// tangent angle (radians) of the segment it falls on. `cum` is the
/// cumulative length per vertex (`cum[0] == 0`).
fn point_and_angle_at(pts: &[(f32, f32)], cum: &[f32], dist: f32) -> ((f32, f32), f32) {
    let total = *cum.last().unwrap();
    let d = dist.clamp(0.0, total);
    // Find the segment [i, i+1] containing `d`.
    let mut i = 0;
    while i + 2 < pts.len() && cum[i + 1] < d {
        i += 1;
    }
    let seg = cum[i + 1] - cum[i];
    let t = if seg > 1e-6 { (d - cum[i]) / seg } else { 0.0 };
    let (ax, ay) = pts[i];
    let (bx, by) = pts[i + 1];
    let p = (ax + (bx - ax) * t, ay + (by - ay) * t);
    let angle = (by - ay).atan2(bx - ax);
    (p, angle)
}

/// A 2D axis-aligned bounding box in screen pixels.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Aabb {
    pub min_x: f32,
    pub min_y: f32,
    pub max_x: f32,
    pub max_y: f32,
}

impl Aabb {
    pub fn from_glyphs(glyphs: &[LayoutGlyph]) -> Option<Self> {
        let first = glyphs.first()?;
        let mut a = Aabb {
            min_x: first.screen_x,
            min_y: first.screen_y,
            max_x: first.screen_x + first.width,
            max_y: first.screen_y + first.height,
        };
        for g in &glyphs[1..] {
            a.min_x = a.min_x.min(g.screen_x);
            a.min_y = a.min_y.min(g.screen_y);
            a.max_x = a.max_x.max(g.screen_x + g.width);
            a.max_y = a.max_y.max(g.screen_y + g.height);
        }
        Some(a)
    }

    pub fn pad(self, px: f32) -> Self {
        Self {
            min_x: self.min_x - px,
            min_y: self.min_y - px,
            max_x: self.max_x + px,
            max_y: self.max_y + px,
        }
    }

    pub fn overlaps(self, other: Self) -> bool {
        !(self.max_x <= other.min_x
            || self.min_x >= other.max_x
            || self.max_y <= other.min_y
            || self.min_y >= other.max_y)
    }
}

/// Bit-key for a world position so it can live in a `HashSet`. A POI icon
/// and its label come from the same feature point, so their `world_pos`
/// bits match exactly — that's how the icon pass identifies which markers
/// the text pass placed.
pub fn anchor_key(world_pos: (f32, f32)) -> (u32, u32) {
    (world_pos.0.to_bits(), world_pos.1.to_bits())
}

/// Cache of laid-out glyph runs, keyed by `(text, quantised font size)`.
/// Stores anchor-relative layouts (centred around `(0, 0)`); callers
/// translate by the actual screen anchor at draw time, so the same text +
/// size hits the cache regardless of where it lands on screen.
#[derive(Default)]
pub struct LayoutCache {
    entries: std::collections::HashMap<LayoutKey, Vec<LayoutGlyph>>,
}

#[derive(Hash, Eq, PartialEq, Clone)]
struct LayoutKey {
    text: String,
    /// font size in tenths of a pixel — quantises to avoid float-key issues.
    font_size_tenths: u32,
}

impl LayoutCache {
    pub fn new() -> Self {
        Self::default()
    }

    /// Lay `text` out at `font_size_px` around the origin, caching the
    /// result. Subsequent calls with the same key skip layout entirely.
    pub fn get_or_compute(
        &mut self,
        text: &str,
        font_size_px: f32,
        atlas: &mut FontAtlas,
    ) -> &[LayoutGlyph] {
        let key = LayoutKey {
            text: text.to_owned(),
            font_size_tenths: (font_size_px * 10.0).round() as u32,
        };
        self.entries
            .entry(key)
            .or_insert_with(|| layout(text, font_size_px, (0.0, 0.0), atlas))
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

/// Lay out a string centred horizontally around `anchor_px`, with the
/// baseline at `anchor_px.1`. Glyphs are pulled from the atlas (and
/// rasterised on demand).
pub fn layout(
    text: &str,
    font_size_px: f32,
    anchor_px: (f32, f32),
    atlas: &mut FontAtlas,
) -> Vec<LayoutGlyph> {
    let scale = font_size_px / RASTER_PX;
    let glyphs = atlas.shape(text);

    // Total advance (HarfBuzz's, not the raw glyph advance) for centering.
    let total_advance: f32 = glyphs.iter().map(|g| g.x_advance * scale).sum();

    let mut cursor_x = anchor_px.0 - total_advance * 0.5;
    let baseline_y = anchor_px.1;
    let mut out = Vec::with_capacity(glyphs.len());
    for sg in &glyphs {
        if let Some(g) = atlas.ensure(sg.face_id, sg.glyph_id) {
            if g.width > 0 && g.height > 0 {
                out.push(LayoutGlyph {
                    screen_x: cursor_x + (sg.x_offset + g.bearing_x) * scale,
                    screen_y: baseline_y + (g.bearing_y - sg.y_offset) * scale,
                    width: g.width as f32 * scale,
                    height: g.height as f32 * scale,
                    atlas_x: g.atlas_x as f32,
                    atlas_y: g.atlas_y as f32,
                    atlas_w: g.width as f32,
                    atlas_h: g.height as f32,
                });
            }
        }
        cursor_x += sg.x_advance * scale;
    }
    out
}

/// Generate an 8-bit signed-distance field from an alpha mask. Pixels
/// with `alpha >= 128` are treated as "inside" the glyph; the rest are
/// "outside". The output u8 maps signed distance to bytes with
/// `SDF_CENTER` (128) meaning "exactly on the contour", lower values
/// inside, higher values outside.
///
/// Uses the 8-point Sequential Sweeping Euclidean Distance Transform
/// (8SSEDT): two passes per grid, two grids (one for outside-distance,
/// one for inside-distance). O(n) per glyph at insert time.
pub fn generate_sdf(alpha: &[u8], width: u32, height: u32, radius_px: f32) -> Vec<u8> {
    let w = width as usize;
    let h = height as usize;
    let n = w * h;
    debug_assert_eq!(alpha.len(), n);

    // Each grid stores a displacement (dx, dy) from the pixel to its
    // nearest target-set member. `(0, 0)` = "this pixel is in the target
    // set"; anything else means "the nearest target is `dx, dy` away".
    let inf = (i16::MAX, i16::MAX);
    let mut outer = vec![inf; n]; // target set = inside pixels
    let mut inner = vec![inf; n]; // target set = outside pixels

    for (i, a) in alpha.iter().enumerate() {
        if *a >= 128 {
            outer[i] = (0, 0);
        } else {
            inner[i] = (0, 0);
        }
    }

    distance_transform(&mut outer, w, h);
    distance_transform(&mut inner, w, h);

    let mut sdf = vec![SDF_CENTER; n];
    let scale = 128.0 / radius_px; // u8 units per pixel
    for i in 0..n {
        let outer_dist = ((outer[i].0 as f32).powi(2) + (outer[i].1 as f32).powi(2)).sqrt();
        let inner_dist = ((inner[i].0 as f32).powi(2) + (inner[i].1 as f32).powi(2)).sqrt();
        // Positive distance outside, negative inside.
        let signed = outer_dist - inner_dist;
        let mapped = (SDF_CENTER as f32 + signed * scale).round();
        sdf[i] = mapped.clamp(0.0, 255.0) as u8;
    }
    sdf
}

/// 8SSEDT distance transform — propagates displacement vectors across the
/// grid in two sweeps (forward then backward), updating each pixel from
/// its 4 prior-pass neighbours.
fn distance_transform(grid: &mut [(i16, i16)], w: usize, h: usize) {
    let cmp = |grid: &mut [(i16, i16)], idx: usize, src_idx: usize, off_x: i16, off_y: i16| {
        let other = grid[src_idx];
        if other.0 == i16::MAX {
            return;
        }
        let cand_x = other.0.saturating_add(off_x);
        let cand_y = other.1.saturating_add(off_y);
        let cand_d2 = (cand_x as i32) * (cand_x as i32) + (cand_y as i32) * (cand_y as i32);
        let cur_d2 = if grid[idx].0 == i16::MAX {
            i32::MAX
        } else {
            (grid[idx].0 as i32) * (grid[idx].0 as i32)
                + (grid[idx].1 as i32) * (grid[idx].1 as i32)
        };
        if cand_d2 < cur_d2 {
            grid[idx] = (cand_x, cand_y);
        }
    };

    // Forward sweep: top-left → bottom-right, check N/NW/NE/W neighbours.
    for y in 0..h {
        for x in 0..w {
            let idx = y * w + x;
            if y > 0 {
                if x > 0 {
                    cmp(grid, idx, idx - w - 1, 1, 1);
                }
                cmp(grid, idx, idx - w, 0, 1);
                if x + 1 < w {
                    cmp(grid, idx, idx - w + 1, -1, 1);
                }
            }
            if x > 0 {
                cmp(grid, idx, idx - 1, 1, 0);
            }
        }
    }

    // Backward sweep: bottom-right → top-left, check E/SE/S/SW neighbours.
    for y in (0..h).rev() {
        for x in (0..w).rev() {
            let idx = y * w + x;
            if x + 1 < w {
                cmp(grid, idx, idx + 1, -1, 0);
            }
            if y + 1 < h {
                if x + 1 < w {
                    cmp(grid, idx, idx + w + 1, -1, -1);
                }
                cmp(grid, idx, idx + w, 0, -1);
                if x > 0 {
                    cmp(grid, idx, idx + w - 1, 1, -1);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    //! Value boundary: a developer writing a host or a style consumes
    //! `FontAtlas` + `layout` and expects (a) the atlas memoises glyphs (so
    //! frame-rate cost is bounded), (b) layout produces glyphs in reading
    //! order with monotonically advancing cursor, (c) AABB collision
    //! correctly de-duplicates overlapping labels.
    use super::*;

    /// Test helper: shape a single char and ensure its glyph.
    fn ensure_char(atlas: &mut FontAtlas, c: char) -> Option<GlyphInfo> {
        let sg = atlas.shape(&c.to_string())[0];
        atlas.ensure(sg.face_id, sg.glyph_id)
    }

    #[test]
    fn ensure_memoises_glyphs() {
        let mut atlas = FontAtlas::new();
        let a1 = ensure_char(&mut atlas, 'A').unwrap();
        let a2 = ensure_char(&mut atlas, 'A').unwrap();
        // Same glyph cached at the same atlas coords.
        assert_eq!(a1.atlas_x, a2.atlas_x);
        assert_eq!(a1.atlas_y, a2.atlas_y);
        assert_eq!(a1.width, a2.width);
    }

    #[test]
    fn shelf_packing_places_distinct_glyphs_in_non_overlapping_atlas_rects() {
        let mut atlas = FontAtlas::new();
        let entries: Vec<GlyphInfo> = "ABCDEFG"
            .chars()
            .map(|c| ensure_char(&mut atlas, c).expect("atlas should not be full"))
            .collect();
        // Pairwise: no rect overlaps another. Width 0 (invisible glyphs)
        // aren't tested as they don't get a real slot.
        for i in 0..entries.len() {
            for j in (i + 1)..entries.len() {
                let a = &entries[i];
                let b = &entries[j];
                if a.width == 0 || b.width == 0 {
                    continue;
                }
                let ax = a.atlas_x as i64;
                let ay = a.atlas_y as i64;
                let bx = b.atlas_x as i64;
                let by = b.atlas_y as i64;
                let overlaps = ax < bx + b.width as i64
                    && ax + a.width as i64 > bx
                    && ay < by + b.height as i64
                    && ay + a.height as i64 > by;
                assert!(
                    !overlaps,
                    "glyphs {i} and {j} share atlas pixels: {a:?} vs {b:?}",
                );
            }
        }
    }

    #[test]
    fn fallback_face_covers_cjk_when_default_does_not() {
        // Roboto (face 0) has no CJK glyphs. A host-supplied CJK fallback
        // face must be picked for a Japanese character, while Latin still
        // resolves to face 0 — and the CJK glyph rasterises into the shared
        // atlas. Skips cleanly where no system CJK font is installed.
        const CJK_FONT: &str = "/usr/share/fonts/truetype/fonts-japanese-gothic.ttf";
        let Ok(bytes) = std::fs::read(CJK_FONT) else {
            eprintln!("SKIP: no system CJK font at {CJK_FONT}");
            return;
        };
        let mut atlas = FontAtlas::new();
        assert_eq!(atlas.face_count(), 1, "starts with the bundled default");
        assert!(atlas.add_fallback_face(bytes), "CJK font must parse");
        assert_eq!(atlas.face_count(), 2);

        // Latin 'A' → face 0; the kanji 東 (U+6771) → the fallback face.
        assert_eq!(atlas.shape("A")[0].face_id, 0, "Latin stays on the default face");
        let jp = atlas.shape("東");
        assert_eq!(jp.len(), 1);
        assert_eq!(jp[0].face_id, 1, "CJK falls back to the host face");
        assert_ne!(jp[0].glyph_id.0, 0, "fallback face must have a real glyph");

        // The CJK glyph rasterises into the shared atlas.
        let g = atlas
            .ensure(jp[0].face_id, jp[0].glyph_id)
            .expect("CJK glyph rasterises");
        assert!(g.width > 0 && g.height > 0, "CJK glyph has pixels");

        // Mixed Latin+CJK lays out with a glyph per visible char.
        let glyphs = layout("A東", 24.0, (0.0, 0.0), &mut atlas);
        assert!(glyphs.len() >= 2, "mixed run lays out, got {}", glyphs.len());
    }

    #[test]
    fn complex_scripts_shape_via_fallback_font() {
        // FreeSerif covers Arabic + Devanagari. Proves HarfBuzz shaping runs
        // through the fallback face: Arabic joins (no glyph explosion) and
        // Devanagari produces a non-empty reordered run, both rasterisable.
        const FREESERIF: &str = "/usr/share/fonts/truetype/freefont/FreeSerif.ttf";
        let Ok(bytes) = std::fs::read(FREESERIF) else {
            eprintln!("SKIP: no FreeSerif at {FREESERIF}");
            return;
        };
        let mut atlas = FontAtlas::new();
        assert!(atlas.add_fallback_face(bytes), "FreeSerif must parse");
        let ff = atlas.face_count() - 1;

        // Arabic (RTL + joining). Joining forms never add glyphs.
        let arabic = "مرحبا"; // "marhaban"
        let shaped = atlas.shape(arabic);
        assert!(!shaped.is_empty(), "Arabic shapes to glyphs");
        assert!(
            shaped.iter().all(|g| g.face_id == ff),
            "Arabic uses the covering face"
        );
        assert!(
            shaped.len() <= arabic.chars().count(),
            "joining/ligatures don't add glyphs: {} vs {}",
            shaped.len(),
            arabic.chars().count()
        );

        // Devanagari (reordering/conjuncts) — non-empty run on the face.
        let hindi = atlas.shape("नमस्ते"); // "namaste"
        assert!(!hindi.is_empty(), "Devanagari shapes to glyphs");
        assert!(hindi.iter().all(|g| g.face_id == ff));

        // Every shaped glyph rasterises into the shared atlas.
        for sg in shaped.iter().chain(hindi.iter()) {
            let g = atlas.ensure(sg.face_id, sg.glyph_id).expect("glyph rasterises");
            let _ = g;
        }
    }

    #[test]
    fn bidi_orders_mixed_runs_left_to_right() {
        // A Latin+Latin string stays in logical order; the shaped advance is
        // positive and the run is monotonic — a smoke test that the bidi
        // path doesn't scramble simple LTR text.
        let atlas = FontAtlas::new();
        let g = atlas.shape("Map");
        assert_eq!(g.len(), 3, "3 Latin glyphs");
        assert!(g.iter().all(|s| s.x_advance > 0.0), "advances are positive");
    }

    #[test]
    fn layout_emits_glyphs_in_reading_order_with_monotonic_x() {
        // "Hello" — five visible glyphs, each placed left-to-right.
        let mut atlas = FontAtlas::new();
        let glyphs = layout("Hello", 16.0, (500.0, 300.0), &mut atlas);
        assert_eq!(
            glyphs.len(),
            5,
            "expected 5 visible glyphs for 'Hello', got {}",
            glyphs.len(),
        );
        for w in glyphs.windows(2) {
            assert!(
                w[0].screen_x <= w[1].screen_x,
                "glyphs must advance left-to-right: {:?} then {:?}",
                w[0],
                w[1],
            );
        }
    }

    #[test]
    fn layout_centres_text_horizontally_around_anchor() {
        let mut atlas = FontAtlas::new();
        let anchor = (1000.0_f32, 500.0_f32);
        let glyphs = layout("Oslo", 20.0, anchor, &mut atlas);
        let aabb = Aabb::from_glyphs(&glyphs).unwrap();
        let centre = (aabb.min_x + aabb.max_x) * 0.5;
        // Within one pixel of the anchor — exact equality depends on per-
        // glyph bearings.
        assert!(
            (centre - anchor.0).abs() < 4.0,
            "expected centred around {}, got centre {centre} (aabb {aabb:?})",
            anchor.0,
        );
    }

    #[test]
    fn aabb_overlap_detects_intersecting_rectangles_only() {
        let a = Aabb {
            min_x: 0.0,
            min_y: 0.0,
            max_x: 10.0,
            max_y: 10.0,
        };
        let b_intersecting = Aabb {
            min_x: 5.0,
            min_y: 5.0,
            max_x: 15.0,
            max_y: 15.0,
        };
        let b_touching = Aabb {
            min_x: 10.0,
            min_y: 0.0,
            max_x: 20.0,
            max_y: 10.0,
        }; // shares an edge but doesn't overlap
        let b_disjoint = Aabb {
            min_x: 20.0,
            min_y: 0.0,
            max_x: 30.0,
            max_y: 10.0,
        };
        assert!(a.overlaps(b_intersecting));
        assert!(
            !a.overlaps(b_touching),
            "edge-touching must not count as overlap"
        );
        assert!(!a.overlaps(b_disjoint));
    }

    // ---- layout cache --------------------------------------------------

    #[test]
    fn layout_cache_hit_returns_the_same_glyphs_as_a_miss() {
        let mut atlas = FontAtlas::new();
        let mut cache = LayoutCache::new();
        let first = cache.get_or_compute("Bergen", 14.0, &mut atlas).to_vec();
        assert_eq!(cache.len(), 1);
        let second = cache.get_or_compute("Bergen", 14.0, &mut atlas).to_vec();
        // Cache size unchanged on the second call.
        assert_eq!(cache.len(), 1);
        assert_eq!(first, second);
    }

    #[test]
    fn layout_cache_distinct_keys_for_distinct_text() {
        let mut atlas = FontAtlas::new();
        let mut cache = LayoutCache::new();
        cache.get_or_compute("Bergen", 14.0, &mut atlas);
        cache.get_or_compute("Oslo", 14.0, &mut atlas);
        assert_eq!(cache.len(), 2);
    }

    #[test]
    fn layout_cache_distinct_keys_for_distinct_sizes() {
        let mut atlas = FontAtlas::new();
        let mut cache = LayoutCache::new();
        cache.get_or_compute("Bergen", 14.0, &mut atlas);
        cache.get_or_compute("Bergen", 16.0, &mut atlas);
        assert_eq!(cache.len(), 2);
    }

    #[test]
    fn layout_cache_quantises_font_size_to_tenths() {
        // 14.00 and 14.04 round to the same tenth (14.0), so they share
        // a cache entry. 14.05 rounds to 14.1, separate entry.
        let mut atlas = FontAtlas::new();
        let mut cache = LayoutCache::new();
        cache.get_or_compute("Bergen", 14.00, &mut atlas);
        cache.get_or_compute("Bergen", 14.04, &mut atlas);
        assert_eq!(cache.len(), 1, "same tenth ⇒ same entry");
        cache.get_or_compute("Bergen", 14.10, &mut atlas);
        assert_eq!(cache.len(), 2, "different tenth ⇒ new entry");
    }

    #[test]
    fn layout_cache_glyphs_are_anchor_relative() {
        // Layout stored in the cache should be centred around (0, 0) so
        // callers can translate by the actual anchor at draw time without
        // re-running layout.
        let mut atlas = FontAtlas::new();
        let mut cache = LayoutCache::new();
        let glyphs = cache.get_or_compute("Hi", 16.0, &mut atlas).to_vec();
        let aabb = Aabb::from_glyphs(&glyphs).expect("glyphs present");
        let centre = (aabb.min_x + aabb.max_x) * 0.5;
        assert!(centre.abs() < 4.0, "expected ~0 centre, got {centre}");
    }

    // ---- along-path layout ---------------------------------------------

    #[test]
    fn along_path_straight_horizontal_line_has_no_rotation() {
        // A long horizontal line: glyphs advance left-to-right, all angle 0,
        // x strictly increasing.
        let mut atlas = FontAtlas::new();
        let path = [(0.0, 100.0), (400.0, 100.0)];
        let glyphs = layout_along_path("MAIN ST", 16.0, &path, &mut atlas).expect("fits");
        assert!(!glyphs.is_empty());
        for g in &glyphs {
            assert!(g.angle.abs() < 1e-4, "horizontal line ⇒ angle ~0, got {}", g.angle);
        }
        for w in glyphs.windows(2) {
            assert!(
                w[1].pivot.0 >= w[0].pivot.0,
                "pen must advance along +x: {} then {}",
                w[0].pivot.0,
                w[1].pivot.0
            );
        }
    }

    #[test]
    fn along_path_rejects_text_longer_than_line() {
        // A 5px line can't hold a word — the label is dropped, not overflowed.
        let mut atlas = FontAtlas::new();
        let path = [(0.0, 0.0), (5.0, 0.0)];
        assert!(layout_along_path("LONGROADNAME", 18.0, &path, &mut atlas).is_none());
    }

    #[test]
    fn along_path_follows_a_bend_with_varying_angles() {
        // An L-shaped path: glyphs on the vertical leg must be rotated
        // roughly ±90°, proving they track the tangent, not a fixed axis.
        let mut atlas = FontAtlas::new();
        let path = [(0.0, 0.0), (300.0, 0.0), (300.0, 300.0)];
        let glyphs = layout_along_path("CORNER ROAD", 16.0, &path, &mut atlas).expect("fits");
        let max_angle = glyphs.iter().map(|g| g.angle.abs()).fold(0.0_f32, f32::max);
        assert!(
            max_angle > 1.0,
            "glyphs past the bend should rotate towards vertical, max |angle| = {max_angle}"
        );
    }

    #[test]
    fn along_path_westbound_line_is_not_upside_down() {
        // Same geometry, opposite winding. Reversing for readability means
        // the first glyph still lands near the western (small-x) end.
        let mut atlas = FontAtlas::new();
        let eastbound = [(0.0, 50.0), (400.0, 50.0)];
        let westbound = [(400.0, 50.0), (0.0, 50.0)];
        let a = layout_along_path("ROUTE", 16.0, &eastbound, &mut atlas).unwrap();
        let b = layout_along_path("ROUTE", 16.0, &westbound, &mut atlas).unwrap();
        // First glyph pen x should be on the western half both times.
        assert!(a[0].pivot.0 < 200.0);
        assert!(b[0].pivot.0 < 200.0);
        for g in a.iter().chain(b.iter()) {
            assert!(g.angle.abs() < 1e-4, "both readings stay horizontal");
        }
    }

    // ---- SDF generator -------------------------------------------------

    /// Build a small square alpha mask: 6x6 with the centre 2x2 fully
    /// opaque, everything else fully transparent.
    fn square_mask() -> (Vec<u8>, u32, u32) {
        let w = 6;
        let h = 6;
        let mut m = vec![0u8; (w * h) as usize];
        for y in 2..=3 {
            for x in 2..=3 {
                m[(y * w + x) as usize] = 255;
            }
        }
        (m, w, h)
    }

    #[test]
    fn sdf_centre_pixel_inside_shape_is_below_centre_value() {
        // A pixel deep inside the shape should encode as < 128 (negative
        // signed distance ⇒ inside).
        let (mask, w, h) = square_mask();
        let sdf = generate_sdf(&mask, w, h, 4.0);
        // The very centre of our 2x2 "inside" cluster.
        let idx = (2 * w + 2) as usize;
        assert!(
            sdf[idx] <= SDF_CENTER,
            "inside pixel should be <= centre, got {}",
            sdf[idx],
        );
    }

    #[test]
    fn sdf_pixel_outside_shape_is_above_centre_value() {
        // A pixel far from any "inside" pixel should encode as > 128.
        let (mask, w, h) = square_mask();
        let sdf = generate_sdf(&mask, w, h, 4.0);
        let idx = 0; // corner — far from the centre cluster
        assert!(
            sdf[idx] > SDF_CENTER,
            "outside pixel should be > centre, got {}",
            sdf[idx],
        );
    }

    #[test]
    fn sdf_distance_monotonically_increases_away_from_shape() {
        // Moving away from the inside region, SDF values should monotonically
        // increase. Walk along a row from the inside cluster outward.
        let (mask, w, _h) = square_mask();
        let sdf = generate_sdf(&mask, w, 6, 4.0);
        let row_y = 2_u32; // y where the inside row sits
        let mut last = -1_i32;
        for x in 3..=5u32 {
            let v = sdf[(row_y * w + x) as usize] as i32;
            assert!(v >= last, "non-monotone at x={x}: {v} < {last}");
            last = v;
        }
    }

    #[test]
    fn sdf_empty_mask_yields_uniform_far_outside() {
        // No "inside" pixels at all — every pixel's outside distance is
        // infinite, so the encoded value clamps to 255.
        let mask = vec![0u8; 16];
        let sdf = generate_sdf(&mask, 4, 4, 4.0);
        for &v in &sdf {
            assert_eq!(v, 255, "no-target mask must encode as max outside");
        }
    }

    #[test]
    fn atlas_gutter_is_outside_value() {
        // The atlas must clear to 255 ("fully outside the glyph"), not 0
        // ("deep inside"). A zeroed gutter makes the bilinear sampler ramp
        // 255→0 at each glyph cell edge, crossing the fill+halo thresholds
        // and drawing a box around every glyph. Sampling an untouched
        // corner of a fresh atlas must read as outside.
        let atlas = FontAtlas::new();
        let bm = atlas.bitmap();
        // Bottom-right corner is never written by the shelf packer.
        let last = (ATLAS_SIZE * ATLAS_SIZE - 1) as usize;
        assert_eq!(bm[last], 255, "unwritten atlas must read as outside");
    }
}
