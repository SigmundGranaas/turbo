//! Just enough GeoTIFF to read what Kartverket's WCS returns.
//!
//! Not a general TIFF reader, and deliberately so. A general reader is a
//! large dependency and a large attack surface for one narrow job: the
//! WCS answers `GetCoverage` with an uncompressed, tiled, single-band
//! float32 image, and every field this needs is in the first IFD.
//!
//! What it *does* insist on is that the file is that. A silently
//! mis-decoded DEM is the worst failure this pipeline has — it does not
//! throw, it produces terrain that is wrong in a plausible-looking way,
//! and the router happily walks it. So every assumption is checked and
//! anything unexpected is an error rather than a best effort.

use std::collections::HashMap;

use byteorder::{BigEndian, ByteOrder, LittleEndian};

use crate::BuildError;

/// One decoded coverage: a north-up float32 grid in projected metres.
#[derive(Debug, Clone)]
pub struct Raster {
    /// Easting of the west edge of column 0.
    pub origin_x: f64,
    /// Northing of the north edge of row 0.
    pub origin_y: f64,
    pub pixel_size_x: f64,
    /// Positive; rows run *southward* from [`Self::origin_y`].
    pub pixel_size_y: f64,
    pub width: usize,
    pub height: usize,
    /// Row-major, `width * height` samples.
    pub values: Vec<f32>,
}

// TIFF tags this reader understands.
const T_WIDTH: u16 = 256;
const T_HEIGHT: u16 = 257;
const T_BITS: u16 = 258;
const T_COMPRESSION: u16 = 259;
const T_SAMPLES_PER_PIXEL: u16 = 277;
const T_ROWS_PER_STRIP: u16 = 278;
const T_STRIP_OFFSETS: u16 = 273;
const T_STRIP_BYTE_COUNTS: u16 = 279;
const T_TILE_WIDTH: u16 = 322;
const T_TILE_HEIGHT: u16 = 323;
const T_TILE_OFFSETS: u16 = 324;
const T_TILE_BYTE_COUNTS: u16 = 325;
const T_SAMPLE_FORMAT: u16 = 339;
const T_PIXEL_SCALE: u16 = 33550;
const T_TIE_POINT: u16 = 33922;

const SAMPLE_FORMAT_IEEE_FP: u64 = 3;
const COMPRESSION_NONE: u64 = 1;

struct Entry {
    kind: u16,
    count: u64,
    /// Raw 4-byte value field; either the value itself or an offset.
    value_field: u32,
}

/// Byte order is decided by the file's own first two bytes.
enum Order {
    Le,
    Be,
}

impl Order {
    fn u16(&self, b: &[u8]) -> u16 {
        match self {
            Order::Le => LittleEndian::read_u16(b),
            Order::Be => BigEndian::read_u16(b),
        }
    }
    fn u32(&self, b: &[u8]) -> u32 {
        match self {
            Order::Le => LittleEndian::read_u32(b),
            Order::Be => BigEndian::read_u32(b),
        }
    }
    fn f64(&self, b: &[u8]) -> f64 {
        match self {
            Order::Le => LittleEndian::read_f64(b),
            Order::Be => BigEndian::read_f64(b),
        }
    }
    fn f32(&self, b: &[u8]) -> f32 {
        match self {
            Order::Le => LittleEndian::read_f32(b),
            Order::Be => BigEndian::read_f32(b),
        }
    }
}

fn err(msg: impl Into<String>) -> BuildError {
    BuildError::Decode(msg.into())
}

fn need(d: &[u8], at: usize, n: usize) -> Result<&[u8], BuildError> {
    d.get(at..at + n).ok_or_else(|| {
        err(format!(
            "truncated at byte {at} (+{n}), file is {} bytes",
            d.len()
        ))
    })
}

/// Decode a single-band, uncompressed, float32 GeoTIFF.
pub fn decode(data: &[u8]) -> Result<Raster, BuildError> {
    if data.len() < 8 {
        return Err(err("not a TIFF: shorter than a header"));
    }
    let order = match &data[0..2] {
        b"II" => Order::Le,
        b"MM" => Order::Be,
        // The WCS returns an HTML error page with a 200 in some failure
        // modes, so say what it actually looks like rather than "bad
        // magic" — that message has cost people hours.
        other => {
            let head = String::from_utf8_lossy(&data[..data.len().min(80)]);
            return Err(err(format!(
                "not a TIFF (byte-order mark {other:?}); body begins: {head:?}"
            )));
        }
    };
    if order.u16(&data[2..4]) != 42 {
        return Err(err("not a TIFF: bad version"));
    }

    let ifd = order.u32(&data[4..8]) as usize;
    let count = order.u16(need(data, ifd, 2)?) as usize;
    let mut tags: HashMap<u16, Entry> = HashMap::with_capacity(count);
    for i in 0..count {
        let at = ifd + 2 + i * 12;
        let b = need(data, at, 12)?;
        tags.insert(
            order.u16(&b[0..2]),
            Entry {
                kind: order.u16(&b[2..4]),
                count: order.u32(&b[4..8]) as u64,
                value_field: order.u32(&b[8..12]),
            },
        );
    }

    // Scalar tag reads. Values of 4 bytes or fewer live in the entry
    // itself; longer ones are at an offset. For the scalars this reader
    // wants, count is always 1, so the value is always inline.
    let scalar = |t: u16| -> Option<u64> {
        tags.get(&t).map(|e| match e.kind {
            // SHORT is stored in the *high* half on big-endian files.
            3 => match order {
                Order::Le => (e.value_field & 0xFFFF) as u64,
                Order::Be => (e.value_field >> 16) as u64,
            },
            _ => e.value_field as u64,
        })
    };

    let width = scalar(T_WIDTH).ok_or_else(|| err("no ImageWidth"))? as usize;
    let height = scalar(T_HEIGHT).ok_or_else(|| err("no ImageLength"))? as usize;
    let bits = scalar(T_BITS).unwrap_or(0);
    let samples = scalar(T_SAMPLES_PER_PIXEL).unwrap_or(1);
    let compression = scalar(T_COMPRESSION).unwrap_or(COMPRESSION_NONE);
    let fmt = scalar(T_SAMPLE_FORMAT).unwrap_or(0);

    if bits != 32 || fmt != SAMPLE_FORMAT_IEEE_FP {
        return Err(err(format!(
            "expected 32-bit IEEE float samples, got {bits}-bit format {fmt}"
        )));
    }
    if samples != 1 {
        return Err(err(format!("expected one band, got {samples}")));
    }
    if compression != COMPRESSION_NONE {
        return Err(err(format!(
            "expected uncompressed samples, got compression {compression}"
        )));
    }

    // Georeferencing. ModelPixelScale is 3 doubles, ModelTiepoint is 6:
    // (i, j, k, x, y, z) mapping raster point (i, j) to world (x, y).
    let doubles = |t: u16, want: usize| -> Result<Vec<f64>, BuildError> {
        let e = tags.get(&t).ok_or_else(|| err(format!("no tag {t}")))?;
        if (e.count as usize) < want {
            return Err(err(format!("tag {t} has {} values, want {want}", e.count)));
        }
        let at = e.value_field as usize;
        let b = need(data, at, want * 8)?;
        Ok((0..want).map(|i| order.f64(&b[i * 8..])).collect())
    };
    let scale = doubles(T_PIXEL_SCALE, 3)?;
    let tie = doubles(T_TIE_POINT, 6)?;
    if scale[0] <= 0.0 || scale[1] <= 0.0 {
        return Err(err(format!("non-positive pixel scale {:?}", &scale[..2])));
    }
    // A tie point that is not the raster origin would mean the image is
    // offset from the world point given, which this reader does not
    // handle — better to refuse than to place terrain in the wrong spot.
    if tie[0] != 0.0 || tie[1] != 0.0 {
        return Err(err(format!(
            "tie point is at raster ({}, {}), expected (0, 0)",
            tie[0], tie[1]
        )));
    }
    let origin_x = tie[3];
    let origin_y = tie[4];

    // Offsets/counts may be SHORT or LONG, inline or out of line.
    let offsets = |t: u16| -> Result<Vec<u64>, BuildError> {
        let e = tags.get(&t).ok_or_else(|| err(format!("no tag {t}")))?;
        let n = e.count as usize;
        let short = e.kind == 3;
        let stride = if short { 2 } else { 4 };
        if n * stride <= 4 {
            let mut b = [0u8; 4];
            match order {
                Order::Le => LittleEndian::write_u32(&mut b, e.value_field),
                Order::Be => BigEndian::write_u32(&mut b, e.value_field),
            }
            return Ok((0..n)
                .map(|i| {
                    if short {
                        order.u16(&b[i * 2..]) as u64
                    } else {
                        order.u32(&b[i * 4..]) as u64
                    }
                })
                .collect());
        }
        let at = e.value_field as usize;
        let b = need(data, at, n * stride)?;
        Ok((0..n)
            .map(|i| {
                if short {
                    order.u16(&b[i * 2..]) as u64
                } else {
                    order.u32(&b[i * 4..]) as u64
                }
            })
            .collect())
    };

    let mut values = vec![f32::NAN; width * height];

    if tags.contains_key(&T_TILE_OFFSETS) {
        // Tiled — what the Kartverket WCS actually returns.
        let tw = scalar(T_TILE_WIDTH).ok_or_else(|| err("tiled but no TileWidth"))? as usize;
        let th = scalar(T_TILE_HEIGHT).ok_or_else(|| err("tiled but no TileLength"))? as usize;
        if tw == 0 || th == 0 {
            return Err(err("zero tile size"));
        }
        let across = width.div_ceil(tw);
        let offs = offsets(T_TILE_OFFSETS)?;
        let cnts = offsets(T_TILE_BYTE_COUNTS)?;
        if offs.len() != cnts.len() {
            return Err(err("tile offset/bytecount length mismatch"));
        }
        for (i, (&off, &cnt)) in offs.iter().zip(cnts.iter()).enumerate() {
            // A zero-length tile is TIFF's sparse convention for "no
            // data stored here", and Kartverket's WCS uses it: a 159 x
            // 1024 coverage came back with tiles 1 and 3 at offset 0,
            // count 0. Leaving those samples untouched keeps them NaN,
            // which the DEM writer turns into the nodata sentinel. Any
            // other reading — zeros, or an error — would put sea level
            // or a failure where the answer is simply "unknown".
            if cnt == 0 || off == 0 {
                continue;
            }
            let b = need(data, off as usize, cnt as usize)?;
            let tx = (i % across) * tw;
            let ty = (i / across) * th;
            // Tiles are padded to full size at the right and bottom
            // edges; the padding is outside the image and dropped here.
            for r in 0..th {
                let y = ty + r;
                if y >= height {
                    break;
                }
                for c in 0..tw {
                    let x = tx + c;
                    if x >= width {
                        break;
                    }
                    let at = (r * tw + c) * 4;
                    if at + 4 > b.len() {
                        return Err(err(format!(
                            "tile {i} of {} is {} bytes, but a {tw}x{th} float32 tile needs {}; \
                             image is {width}x{height}",
                            offs.len(),
                            b.len(),
                            tw * th * 4
                        )));
                    }
                    values[y * width + x] = order.f32(&b[at..]);
                }
            }
        }
    } else {
        // Stripped. Not what the WCS sends today, but cheap to support
        // and the alternative is a decoder that breaks if it ever does.
        let rps = scalar(T_ROWS_PER_STRIP).unwrap_or(height as u64) as usize;
        if rps == 0 {
            return Err(err("zero RowsPerStrip"));
        }
        let offs = offsets(T_STRIP_OFFSETS)?;
        let cnts = offsets(T_STRIP_BYTE_COUNTS)?;
        if offs.len() != cnts.len() {
            return Err(err("strip offset/bytecount length mismatch"));
        }
        for (i, (&off, &cnt)) in offs.iter().zip(cnts.iter()).enumerate() {
            // Sparse strips, same convention as sparse tiles above.
            if cnt == 0 || off == 0 {
                continue;
            }
            let b = need(data, off as usize, cnt as usize)?;
            for r in 0..rps {
                let y = i * rps + r;
                if y >= height {
                    break;
                }
                for x in 0..width {
                    let at = (r * width + x) * 4;
                    if at + 4 > b.len() {
                        break;
                    }
                    values[y * width + x] = order.f32(&b[at..]);
                }
            }
        }
    }

    Ok(Raster {
        origin_x,
        origin_y,
        pixel_size_x: scale[0],
        pixel_size_y: scale[1],
        width,
        height,
        values,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a tiled float32 GeoTIFF in memory, shaped like the WCS's.
    fn synth(width: usize, height: usize, tile: usize, f: impl Fn(usize, usize) -> f32) -> Vec<u8> {
        let across = width.div_ceil(tile);
        let down = height.div_ceil(tile);
        let ntiles = across * down;
        let tile_bytes = tile * tile * 4;

        let mut out = Vec::new();
        out.extend_from_slice(b"II");
        out.extend_from_slice(&42u16.to_le_bytes());
        out.extend_from_slice(&0u32.to_le_bytes()); // IFD offset, patched

        let scale_at = out.len();
        for v in [10.0f64, 10.0, 0.0] {
            out.extend_from_slice(&v.to_le_bytes());
        }
        let tie_at = out.len();
        for v in [0.0f64, 0.0, 0.0, 480_000.0, 7_425_120.0, 0.0] {
            out.extend_from_slice(&v.to_le_bytes());
        }
        let offs_at = out.len();
        out.extend_from_slice(&vec![0u8; ntiles * 4]);
        let cnts_at = out.len();
        out.extend_from_slice(&vec![0u8; ntiles * 4]);

        let mut offs = Vec::new();
        for t in 0..ntiles {
            offs.push(out.len() as u32);
            let tx = (t % across) * tile;
            let ty = (t / across) * tile;
            for r in 0..tile {
                for c in 0..tile {
                    out.extend_from_slice(&f(tx + c, ty + r).to_le_bytes());
                }
            }
        }
        for (i, o) in offs.iter().enumerate() {
            out[offs_at + i * 4..offs_at + i * 4 + 4].copy_from_slice(&o.to_le_bytes());
            out[cnts_at + i * 4..cnts_at + i * 4 + 4]
                .copy_from_slice(&(tile_bytes as u32).to_le_bytes());
        }

        let ifd_at = out.len();
        out[4..8].copy_from_slice(&(ifd_at as u32).to_le_bytes());
        let entries: Vec<(u16, u16, u32, u32)> = vec![
            (T_WIDTH, 3, 1, width as u32),
            (T_HEIGHT, 3, 1, height as u32),
            (T_BITS, 3, 1, 32),
            (T_COMPRESSION, 3, 1, 1),
            (T_SAMPLES_PER_PIXEL, 3, 1, 1),
            (T_TILE_WIDTH, 3, 1, tile as u32),
            (T_TILE_HEIGHT, 3, 1, tile as u32),
            (T_TILE_OFFSETS, 4, ntiles as u32, offs_at as u32),
            (T_TILE_BYTE_COUNTS, 4, ntiles as u32, cnts_at as u32),
            (T_SAMPLE_FORMAT, 3, 1, 3),
            (T_PIXEL_SCALE, 12, 3, scale_at as u32),
            (T_TIE_POINT, 12, 6, tie_at as u32),
        ];
        out.extend_from_slice(&(entries.len() as u16).to_le_bytes());
        for (tag, kind, count, val) in entries {
            out.extend_from_slice(&tag.to_le_bytes());
            out.extend_from_slice(&kind.to_le_bytes());
            out.extend_from_slice(&count.to_le_bytes());
            // SHORT scalars sit in the low half on little-endian.
            out.extend_from_slice(&val.to_le_bytes());
        }
        out.extend_from_slice(&0u32.to_le_bytes());
        out
    }

    #[test]
    fn decodes_a_tiled_float_raster() {
        let bytes = synth(512, 512, 128, |x, y| (x * 1000 + y) as f32);
        let r = decode(&bytes).expect("decode");
        assert_eq!((r.width, r.height), (512, 512));
        assert_eq!((r.pixel_size_x, r.pixel_size_y), (10.0, 10.0));
        assert_eq!((r.origin_x, r.origin_y), (480_000.0, 7_425_120.0));
        // Spot-check across tile boundaries, which is where a wrong
        // `across` or a wrong stride shows up.
        for (x, y) in [(0, 0), (127, 127), (128, 128), (300, 11), (511, 511)] {
            assert_eq!(
                r.values[y * 512 + x],
                (x * 1000 + y) as f32,
                "at ({x}, {y})"
            );
        }
    }

    /// Edge tiles are padded; the padding must not become terrain.
    #[test]
    fn drops_tile_padding_outside_the_image() {
        let bytes = synth(300, 200, 128, |x, y| (x + y) as f32);
        let r = decode(&bytes).expect("decode");
        assert_eq!((r.width, r.height), (300, 200));
        assert_eq!(r.values.len(), 300 * 200);
        assert_eq!(r.values[199 * 300 + 299], (299 + 199) as f32);
    }

    /// The failure that matters: an HTML error page served with a 200.
    /// Kartverket's WCS returns sparse tiles — offset 0, count 0 — for
    /// ground it has no data for. Observed on a real 159 x 1024
    /// coverage, tiles 1 and 3. Those samples must come back as NaN so
    /// the DEM writer records nodata; decoding them as zeros would put
    /// sea level on a mountainside and the router would walk it.
    #[test]
    fn a_sparse_tile_leaves_its_samples_unknown() {
        let mut bytes = synth(256, 256, 128, |_, _| 500.0);
        let ifd = u32::from_le_bytes(bytes[4..8].try_into().unwrap()) as usize;
        let n = u16::from_le_bytes(bytes[ifd..ifd + 2].try_into().unwrap()) as usize;
        let (mut offs_at, mut cnts_at) = (0usize, 0usize);
        for i in 0..n {
            let at = ifd + 2 + i * 12;
            match u16::from_le_bytes(bytes[at..at + 2].try_into().unwrap()) {
                T_TILE_OFFSETS => {
                    offs_at =
                        u32::from_le_bytes(bytes[at + 8..at + 12].try_into().unwrap()) as usize
                }
                T_TILE_BYTE_COUNTS => {
                    cnts_at =
                        u32::from_le_bytes(bytes[at + 8..at + 12].try_into().unwrap()) as usize
                }
                _ => {}
            }
        }
        // Blank tile 1 (the north-east quadrant).
        bytes[offs_at + 4..offs_at + 8].copy_from_slice(&0u32.to_le_bytes());
        bytes[cnts_at + 4..cnts_at + 8].copy_from_slice(&0u32.to_le_bytes());

        let r = decode(&bytes).expect("a sparse tile must not be an error");
        assert_eq!(r.values[0], 500.0, "present tile lost its data");
        assert!(
            r.values[200].is_nan(),
            "sparse tile decoded as {}",
            r.values[200]
        );
        assert_eq!(
            r.values[130 * 256],
            500.0,
            "tile below the hole lost its data"
        );
    }

    #[test]
    fn an_html_error_page_is_not_silently_decoded() {
        let e = decode(b"<html><head><title>ArcGIS Server Error</title>").unwrap_err();
        let m = e.to_string();
        assert!(m.contains("not a TIFF"), "{m}");
        assert!(m.contains("ArcGIS"), "the message must quote the body: {m}");
    }

    #[test]
    fn refuses_integer_samples_rather_than_reinterpreting_them() {
        let mut bytes = synth(128, 128, 128, |_, _| 1.0);
        // Flip SampleFormat from IEEE float (3) to unsigned int (1).
        let ifd = u32::from_le_bytes(bytes[4..8].try_into().unwrap()) as usize;
        let n = u16::from_le_bytes(bytes[ifd..ifd + 2].try_into().unwrap()) as usize;
        for i in 0..n {
            let at = ifd + 2 + i * 12;
            if u16::from_le_bytes(bytes[at..at + 2].try_into().unwrap()) == T_SAMPLE_FORMAT {
                bytes[at + 8..at + 12].copy_from_slice(&1u32.to_le_bytes());
            }
        }
        let m = decode(&bytes).unwrap_err().to_string();
        assert!(m.contains("IEEE float"), "{m}");
    }
}
