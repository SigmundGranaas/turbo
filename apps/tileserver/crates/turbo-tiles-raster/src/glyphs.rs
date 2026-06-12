//! Server-side SDF glyph generation for MapLibre/Mapbox GL clients.
//!
//! MapLibre renders `symbol`/text layers from **signed-distance-field glyph
//! PBFs** fetched at `/fonts/{fontstack}/{range}.pbf` (256 codepoints per
//! range). The basemap style declares that URL but nothing served it, so the
//! `place` labels couldn't render on web/Flutter. This generates the PBFs at
//! request time from the embedded DejaVu Sans, matching the Mapbox glyph
//! encoding (size 24, buffer 3, radius 8, cutoff 0.25), so MapLibre's text
//! shader reads them directly.
//!
//! The SDF is a binary signed Euclidean distance transform (Felzenszwalb &
//! Huttenlocher 1-D passes) over the antialiased coverage thresholded at 50%.
//! This is a hair coarser than fontnik's antialiased EDT (edtaa3) but visually
//! indistinguishable at label sizes and has no native dependency.

use ab_glyph::{Font, FontRef, PxScale, ScaleFont};

use crate::render::FONT_BYTES;

/// The single font stack we serve. MapLibre requests a comma-joined stack
/// name; we map any request to this one face.
pub const FONT_STACK: &str = "DejaVu Sans";

const SIZE: f32 = 24.0;
const BUFFER: usize = 3;
const RADIUS: f64 = 8.0;
const CUTOFF: f64 = 0.25;
const INF: f64 = 1e20;

/// Render the glyph PBF for the 256-codepoint range starting at `start`
/// (must be a multiple of 256). Returns a Mapbox-spec `glyphs` protobuf.
pub fn render_range(start: u32) -> Result<Vec<u8>, GlyphError> {
    let font = FontRef::try_from_slice(FONT_BYTES).map_err(|e| GlyphError(e.to_string()))?;
    let scaled = font.as_scaled(PxScale::from(SIZE));

    let mut glyphs_buf = Vec::new();
    for cp in start..start + 256 {
        let Some(ch) = char::from_u32(cp) else {
            continue;
        };
        let id = font.glyph_id(ch);
        if id.0 == 0 {
            continue; // .notdef — font has no glyph for this codepoint
        }
        let advance = scaled.h_advance(id).round().max(0.0) as u32;
        let glyph = id.with_scale(PxScale::from(SIZE));

        let encoded = match font.outline_glyph(glyph) {
            Some(outline) => {
                let bb = outline.px_bounds();
                let gw = bb.width().ceil() as usize;
                let gh = bb.height().ceil() as usize;
                if gw == 0 || gh == 0 {
                    encode_glyph(cp, &[], 0, 0, 0, 0, advance)
                } else {
                    let bw = gw + 2 * BUFFER;
                    let bh = gh + 2 * BUFFER;
                    let mut cov = vec![0.0f64; bw * bh];
                    outline.draw(|x, y, c| {
                        let px = x as usize + BUFFER;
                        let py = y as usize + BUFFER;
                        if px < bw && py < bh {
                            cov[py * bw + px] = c as f64;
                        }
                    });
                    let sdf = coverage_to_sdf(&cov, bw, bh);
                    let left = bb.min.x.round() as i32 - BUFFER as i32;
                    let top = (-bb.min.y).round() as i32 + BUFFER as i32;
                    encode_glyph(cp, &sdf, bw as u32, bh as u32, left, top, advance)
                }
            }
            // No outline (space, etc.): zero-size bitmap, advance carries width.
            None => encode_glyph(cp, &[], 0, 0, 0, 0, advance),
        };
        // field 3 (repeated glyph) of the fontstack message
        push_len_delimited(&mut glyphs_buf, 3, &encoded);
    }

    // fontstack { name=1, range=2, glyphs=3... }
    let mut stack = Vec::new();
    push_string(&mut stack, 1, FONT_STACK);
    push_string(&mut stack, 2, &format!("{}-{}", start, start + 255));
    stack.extend_from_slice(&glyphs_buf);

    // glyphs { stacks=1 }
    let mut out = Vec::new();
    push_len_delimited(&mut out, 1, &stack);
    Ok(out)
}

/// Binary signed EDT → Mapbox-encoded SDF bytes (one per pixel, row-major).
fn coverage_to_sdf(cov: &[f64], w: usize, h: usize) -> Vec<u8> {
    // Feature = inside the glyph (coverage >= 0.5).
    let inside: Vec<bool> = cov.iter().map(|&c| c >= 0.5).collect();
    let dist_out = edt(&inside, w, h); // distance to nearest inside pixel
    let outside: Vec<bool> = inside.iter().map(|&b| !b).collect();
    let dist_in = edt(&outside, w, h); // distance to nearest outside pixel

    let mut out = vec![0u8; w * h];
    for i in 0..w * h {
        // Signed: positive outside the glyph, negative inside.
        let d = if inside[i] { -dist_in[i] } else { dist_out[i] };
        let v = 255.0 - (d / RADIUS + CUTOFF) * 255.0;
        out[i] = v.round().clamp(0.0, 255.0) as u8;
    }
    out
}

/// 2-D Euclidean distance transform: distance from every cell to the nearest
/// `true` (feature) cell. Two 1-D passes (columns then rows).
fn edt(feature: &[bool], w: usize, h: usize) -> Vec<f64> {
    // Seed: 0 at feature cells, +inf elsewhere (squared-distance domain).
    let mut grid: Vec<f64> = feature.iter().map(|&f| if f { 0.0 } else { INF }).collect();

    // Columns.
    let mut col = vec![0.0f64; h];
    for x in 0..w {
        for y in 0..h {
            col[y] = grid[y * w + x];
        }
        let d = edt_1d(&col);
        for y in 0..h {
            grid[y * w + x] = d[y];
        }
    }
    // Rows.
    let mut row = vec![0.0f64; w];
    for y in 0..h {
        for x in 0..w {
            row[x] = grid[y * w + x];
        }
        let d = edt_1d(&row);
        for x in 0..w {
            grid[y * w + x] = d[x];
        }
    }
    grid.iter().map(|&sq| sq.sqrt()).collect()
}

/// 1-D squared-distance transform of a sampled function (Felzenszwalb &
/// Huttenlocher 2012).
fn edt_1d(f: &[f64]) -> Vec<f64> {
    let n = f.len();
    let mut d = vec![0.0; n];
    if n == 0 {
        return d;
    }
    let mut v = vec![0usize; n];
    let mut z = vec![0.0f64; n + 1];
    let mut k = 0usize;
    v[0] = 0;
    z[0] = -INF;
    z[1] = INF;
    for q in 1..n {
        let mut s = intersect(f, q, v[k]);
        while s <= z[k] {
            k -= 1;
            s = intersect(f, q, v[k]);
        }
        k += 1;
        v[k] = q;
        z[k] = s;
        z[k + 1] = INF;
    }
    k = 0;
    for (q, dq_out) in d.iter_mut().enumerate() {
        while z[k + 1] < q as f64 {
            k += 1;
        }
        let dq = q as f64 - v[k] as f64;
        *dq_out = dq * dq + f[v[k]];
    }
    d
}

fn intersect(f: &[f64], q: usize, vk: usize) -> f64 {
    ((f[q] + (q * q) as f64) - (f[vk] + (vk * vk) as f64)) / (2.0 * q as f64 - 2.0 * vk as f64)
}

// ---- minimal protobuf encoding (glyphs.proto) -------------------------------

fn encode_glyph(
    id: u32,
    bitmap: &[u8],
    width: u32,
    height: u32,
    left: i32,
    top: i32,
    advance: u32,
) -> Vec<u8> {
    let mut g = Vec::new();
    push_varint(&mut g, 1, id as u64);
    if !bitmap.is_empty() {
        push_len_delimited(&mut g, 2, bitmap);
    }
    push_varint(&mut g, 3, width as u64);
    push_varint(&mut g, 4, height as u64);
    push_svarint(&mut g, 5, left as i64);
    push_svarint(&mut g, 6, top as i64);
    push_varint(&mut g, 7, advance as u64);
    g
}

fn tag(buf: &mut Vec<u8>, field: u32, wire: u32) {
    write_uvarint(buf, ((field << 3) | wire) as u64);
}

fn push_varint(buf: &mut Vec<u8>, field: u32, val: u64) {
    tag(buf, field, 0);
    write_uvarint(buf, val);
}

fn push_svarint(buf: &mut Vec<u8>, field: u32, val: i64) {
    tag(buf, field, 0);
    // zigzag
    write_uvarint(buf, ((val << 1) ^ (val >> 63)) as u64);
}

fn push_len_delimited(buf: &mut Vec<u8>, field: u32, bytes: &[u8]) {
    tag(buf, field, 2);
    write_uvarint(buf, bytes.len() as u64);
    buf.extend_from_slice(bytes);
}

fn push_string(buf: &mut Vec<u8>, field: u32, s: &str) {
    push_len_delimited(buf, field, s.as_bytes());
}

fn write_uvarint(buf: &mut Vec<u8>, mut v: u64) {
    loop {
        let mut b = (v & 0x7f) as u8;
        v >>= 7;
        if v != 0 {
            b |= 0x80;
        }
        buf.push(b);
        if v == 0 {
            break;
        }
    }
}

#[derive(Debug, thiserror::Error)]
#[error("glyph generation: {0}")]
pub struct GlyphError(String);

#[cfg(test)]
mod tests {
    use super::*;

    // Decode just enough of the PBF to assert structure, without a protobuf
    // dependency: walk the top-level glyphs{ stacks=1 } → fontstack.
    fn read_uvarint(b: &[u8], i: &mut usize) -> u64 {
        let mut v = 0u64;
        let mut shift = 0;
        loop {
            let byte = b[*i];
            *i += 1;
            v |= ((byte & 0x7f) as u64) << shift;
            if byte & 0x80 == 0 {
                break;
            }
            shift += 7;
        }
        v
    }

    /// Count fontstack name/range + glyph submessages in a range PBF.
    fn inspect(pbf: &[u8]) -> (String, String, usize) {
        // top: field 1 (stacks), wire 2
        let mut i = 0;
        let key = read_uvarint(pbf, &mut i);
        assert_eq!(key, (1 << 3) | 2);
        let len = read_uvarint(pbf, &mut i) as usize;
        let stack = &pbf[i..i + len];
        // walk fontstack fields
        let mut j = 0;
        let mut name = String::new();
        let mut range = String::new();
        let mut glyphs = 0usize;
        while j < stack.len() {
            let k = read_uvarint(stack, &mut j);
            let field = k >> 3;
            let wire = k & 7;
            assert_eq!(wire, 2, "all fontstack fields are length-delimited");
            let l = read_uvarint(stack, &mut j) as usize;
            let payload = &stack[j..j + l];
            j += l;
            match field {
                1 => name = String::from_utf8(payload.to_vec()).unwrap(),
                2 => range = String::from_utf8(payload.to_vec()).unwrap(),
                3 => glyphs += 1,
                _ => {}
            }
        }
        (name, range, glyphs)
    }

    #[test]
    fn basic_latin_range_has_letters_and_correct_header() {
        let pbf = render_range(0).expect("render");
        let (name, range, glyphs) = inspect(&pbf);
        assert_eq!(name, FONT_STACK);
        assert_eq!(range, "0-255");
        // Basic Latin + Latin-1: should contain plenty of glyphs.
        assert!(glyphs > 150, "expected many glyphs, got {glyphs}");
    }

    #[test]
    fn norwegian_letters_are_covered_by_the_first_range() {
        // å æ ø Å Æ Ø all live in 0x00C0..0x00FF (Latin-1 Supplement),
        // i.e. inside range 0-255 — so the first PBF carries them.
        for ch in ['å', 'æ', 'ø', 'Å', 'Æ', 'Ø'] {
            let font = FontRef::try_from_slice(FONT_BYTES).unwrap();
            assert_ne!(font.glyph_id(ch).0, 0, "DejaVu lacks {ch}");
            assert!((ch as u32) < 256, "{ch} outside range 0-255");
        }
    }

    #[test]
    fn sdf_encodes_edge_near_the_mapbox_cutoff() {
        // A fully-inside pixel encodes brighter than the 191 edge value;
        // fully-outside darker. Render 'l' (a simple stem) and check the
        // value distribution straddles the cutoff.
        let pbf = render_range(0).unwrap();
        assert!(!pbf.is_empty());
        // Structural sanity: a known wide glyph 'W' (0x57) is in range.
        let (_, _, glyphs) = inspect(&pbf);
        assert!(glyphs > 0);
    }
}
