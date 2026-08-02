//! Trails from the FKB sti WFS, fetched by bbox.
//!
//! # Why the `wms.` host
//!
//! `wfs.geonorge.no` returns 500 for `GetFeature` on this dataset;
//! `wms.geonorge.no` serves it correctly. `turbo-tiles-ingest`'s
//! `fkb_wfs.rs` carries the same workaround with the same note, and the
//! Flutter client did before that. It looks like a typo and is not.
//!
//! # Why grid cells
//!
//! The service caps features per response. A single request for a large
//! bbox comes back *truncated* rather than refused, so the trails simply
//! thin out with no error anywhere — the pack builds, routes, and is
//! missing paths. Chunking into cells keeps each response under the cap,
//! and `numberMatched` vs `numberReturned` is checked per cell so a
//! truncation that does happen is loud.

use geo::Coord;

use crate::gml;
use crate::BuildError;

pub const DEFAULT_ENDPOINT: &str = "https://wms.geonorge.no/skwms1/wms.traktorveg_skogsbilveger";
pub const TYPENAMES: &str = "ms:traktorveg_sti,ms:skogsbilveg";

/// Cell edge in degrees. 0.1° is ~11 km north–south and 4–5 km
/// east–west at 67°N — comfortably inside the per-response cap for
/// trail density anywhere in Norway.
pub const GRID_DEG: f64 = 0.1;

/// A WGS84 bbox, in the order the public endpoints use.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BboxWgs84 {
    pub west: f64,
    pub south: f64,
    pub east: f64,
    pub north: f64,
}

/// Split a bbox into request cells.
pub fn grid_cells(b: BboxWgs84, step: f64) -> Vec<BboxWgs84> {
    let mut out = Vec::new();
    let mut s = b.south;
    while s < b.north {
        let n = (s + step).min(b.north);
        let mut w = b.west;
        while w < b.east {
            let e = (w + step).min(b.east);
            out.push(BboxWgs84 {
                west: w,
                south: s,
                east: e,
                north: n,
            });
            w = e;
        }
        s = n;
    }
    out
}

/// `GetFeature` URL for one cell.
///
/// WFS 2.0.0 with a `urn:` CRS puts BBOX in **lat, lon** order — the
/// axis order the URN implies, not the lon/lat order the same numbers
/// take in a `EPSG:4326` string. Getting this backwards returns an empty
/// response rather than an error, which reads as "no trails here".
pub fn url(endpoint: &str, b: BboxWgs84) -> String {
    format!(
        "{endpoint}?service=WFS&version=2.0.0&request=GetFeature\
         &TYPENAMES={TYPENAMES}&SRSNAME=urn:ogc:def:crs:EPSG::4326\
         &BBOX={},{},{},{},urn:ogc:def:crs:EPSG::4326",
        b.south, b.west, b.north, b.east
    )
}

/// Read `numberMatched` / `numberReturned` off the response envelope.
pub fn counts(xml: &str) -> (Option<usize>, Option<usize>) {
    let get = |k: &str| -> Option<usize> {
        let at = xml.find(k)?;
        let rest = &xml[at + k.len()..];
        let rest = rest.strip_prefix("=\"")?;
        let end = rest.find('"')?;
        rest[..end].parse().ok()
    };
    (get("numberMatched"), get("numberReturned"))
}

/// One trail, already projected into EPSG:25833.
#[derive(Debug, Clone)]
pub struct Trail {
    pub coords: Vec<Coord<f64>>,
    /// `traktorveg_sti` or `skogsbilveg`.
    pub kind: String,
}

/// Fetch one cell and return its trails, projected.
pub fn fetch_cell(
    http: &dyn crate::fetch::Fetch,
    endpoint: &str,
    cell: BboxWgs84,
) -> Result<Vec<Trail>, BuildError> {
    let resp = http.get(&url(endpoint, cell))?;
    if !resp.is_success() {
        return Err(BuildError::Fetch(format!(
            "WFS {}: {}",
            resp.status,
            resp.head(200)
        )));
    }
    let body = String::from_utf8_lossy(&resp.body).into_owned();

    let (matched, returned) = counts(&body);
    if let (Some(m), Some(r)) = (matched, returned) {
        if m > r {
            return Err(BuildError::Fetch(format!(
                "WFS truncated this cell: {r} of {m} features. Reduce GRID_DEG — a \
                 truncated response is silently missing trails, not an error."
            )));
        }
    }

    let mut out = Vec::new();
    gml::read_lines(&body, &["traktorveg_sti", "skogsbilveg"], |f| {
        // The response is in urn:EPSG::4326, so posList pairs are
        // lat lon — the same axis-order rule as the request.
        let coords: Vec<Coord<f64>> = f
            .coords
            .iter()
            .map(|c| {
                let (x, y) = crate::wgs84_to_utm33(c.y, c.x);
                Coord { x, y }
            })
            .collect();
        if coords.len() >= 2 {
            out.push(Trail {
                coords,
                kind: f.kind.clone(),
            });
        }
    })?;
    Ok(out)
}

/// Map a WFS feature name onto the pipeline's `fkb_type` vocabulary.
pub fn fkb_type_of(kind: &str) -> u8 {
    turbo_tiles_graph::encode_fkb_type(Some(match kind {
        "traktorveg_sti" => "sti",
        "skogsbilveg" => "skogsbilveg",
        other => other,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bbox() -> BboxWgs84 {
        BboxWgs84 {
            west: 15.0,
            south: 66.8,
            east: 15.25,
            north: 67.0,
        }
    }

    #[test]
    fn the_grid_covers_the_bbox_without_gaps() {
        let cells = grid_cells(bbox(), GRID_DEG);
        assert!(!cells.is_empty());
        let area: f64 = cells
            .iter()
            .map(|c| (c.east - c.west) * (c.north - c.south))
            .sum();
        let whole = (bbox().east - bbox().west) * (bbox().north - bbox().south);
        assert!(
            (area - whole).abs() < 1e-9,
            "cells cover {area}, bbox is {whole}"
        );
    }

    /// A urn: CRS means lat/lon order. Swapped, the service answers with
    /// zero features and no error — trails silently vanish.
    #[test]
    fn the_bbox_is_lat_lon_for_a_urn_crs() {
        let u = url(DEFAULT_ENDPOINT, bbox());
        assert!(
            u.contains("BBOX=66.8,15,67,15.25,urn:ogc:def:crs:EPSG::4326"),
            "{u}"
        );
    }

    #[test]
    fn reads_the_envelope_counts() {
        let xml = r#"<wfs:FeatureCollection numberMatched="57" numberReturned="57">"#;
        assert_eq!(counts(xml), (Some(57), Some(57)));
    }

    #[test]
    fn a_response_without_counts_is_not_a_parse_failure() {
        assert_eq!(counts("<wfs:FeatureCollection>"), (None, None));
    }

    #[test]
    fn trail_kinds_map_onto_the_shared_vocabulary() {
        // sti must classify as a trail (1), not a road.
        assert_eq!(fkb_type_of("traktorveg_sti"), 1);
        assert_eq!(fkb_type_of("skogsbilveg"), 2);
    }
}
