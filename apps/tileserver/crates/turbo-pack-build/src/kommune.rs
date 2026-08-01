//! Which kommuner does this region touch?
//!
//! N50 has no bbox service. Water, glaciers and roads are ordered per
//! kommune, so a build needs the kommune numbers before it can start.
//! On the CLI a human types them. On a phone the user drags a box on a
//! map and has no idea that Norway has 357 kommuner, let alone which
//! four are under their finger.
//!
//! # Getting this wrong is silent
//!
//! A missing kommune does not fail the build. It produces a pack whose
//! water mask stops at an invisible line, and the router happily plans
//! straight across the lakes on the other side of it. That is the
//! failure this whole pipeline is built to avoid, so the resolution
//! here is deliberately conservative: it would rather download one
//! kommune too many (a wasted 25 MB) than one too few.
//!
//! # The shape
//!
//! Kartverket's Kommuneinfo API has no "kommuner in this box" call, so
//! this composes the calls it does have:
//!
//! 1. **Seed** — a grid of points across the region, each resolved with
//!    `/punkt`.
//! 2. **Grow** — breadth-first over `/nabokommuner`, keeping any
//!    kommune whose *bounding box* meets the region and looking one hop
//!    past every one that does. A neighbour whose box misses is
//!    discarded without being expanded, which is what makes this
//!    terminate quickly instead of walking the country.
//! 3. **Narrow** — the surviving candidates are checked against their
//!    real outline with `/omrade`. Bounding boxes are generous in a
//!    country shaped like this one: a kommune wrapped around a fjord
//!    has a box covering water it does not own, and each false positive
//!    kept would be another 25 MB N50 download for features that cannot
//!    be in the region.
//!
//! Steps 1–2 can only over-collect; step 3 removes what provably does
//! not touch the region. The order matters — narrowing before growing
//! would prune the path to a kommune reachable only through one that
//! does not itself qualify.

use std::collections::{BTreeSet, VecDeque};

use geo::{Contains, Intersects};

use crate::fetch::Fetch;
use crate::n50::Kommune;
use crate::BuildError;

/// Kartverket's Kommuneinfo service.
pub const DEFAULT_ENDPOINT: &str = "https://api.kartverket.no/kommuneinfo/v1";

/// EPSG:4258 (ETRS89) is what the API speaks and what it returns.
///
/// Treated as interchangeable with the WGS84 lon/lat the region is
/// given in. The two differ by centimetres in Norway — irrelevant next
/// to a kommune boundary, and irrelevant to a test that asks only
/// whether two shapes touch at all.
const CRS: &str = "4258";

/// How many points across the region to seed from, per axis.
///
/// Seeds exist to find kommuner that growth cannot reach — an island
/// with no land neighbour, a piece across a fjord. Growth handles
/// everything contiguous, so this does not need to be fine; it needs to
/// be more than one, and cheap. 4x4 is sixteen ~100-byte responses.
const SEED_GRID: usize = 4;

/// Resolve the kommuner overlapping `region` (`[min_lon, min_lat, max_lon, max_lat]`).
///
/// Returns them sorted, so a build of the same region twice produces
/// the same order — the N50 fetch order feeds the surface order, and
/// that feeds the mask.
pub fn resolve(
    fetch: &dyn Fetch,
    region: [f64; 4],
    endpoint: &str,
) -> Result<Vec<Kommune>, BuildError> {
    let rect = geo::Rect::new(
        geo::coord! { x: region[0], y: region[1] },
        geo::coord! { x: region[2], y: region[3] },
    );

    // ---- 1. Seed ----
    let mut queue: VecDeque<String> = VecDeque::new();
    let mut seen: BTreeSet<String> = BTreeSet::new();
    for i in 0..SEED_GRID {
        for j in 0..SEED_GRID {
            // Interior points, not corners: (i + 0.5) / n keeps every
            // sample strictly inside the region, so a box whose edge
            // grazes a neighbouring kommune does not seed from it.
            let fx = (i as f64 + 0.5) / SEED_GRID as f64;
            let fy = (j as f64 + 0.5) / SEED_GRID as f64;
            let lon = region[0] + (region[2] - region[0]) * fx;
            let lat = region[1] + (region[3] - region[1]) * fy;
            if let Some(nr) = punkt(fetch, endpoint, lon, lat)? {
                if seen.insert(nr.clone()) {
                    queue.push_back(nr);
                }
            }
        }
    }
    if queue.is_empty() {
        return Err(BuildError::Logic(format!(
            "no kommune found anywhere in {:.4},{:.4}..{:.4},{:.4} — outside Norway?",
            region[0], region[1], region[2], region[3]
        )));
    }

    // ---- 2. Grow ----
    let mut candidates: Vec<String> = Vec::new();
    while let Some(nr) = queue.pop_front() {
        let Some(bbox) = kommune_bbox(fetch, endpoint, &nr)? else {
            continue;
        };
        if !bbox.intersects(&rect) {
            continue;
        }
        candidates.push(nr.clone());
        for n in naboer(fetch, endpoint, &nr)? {
            if seen.insert(n.clone()) {
                queue.push_back(n);
            }
        }
    }

    // ---- 3. Narrow ----
    let mut kept: Vec<String> = Vec::new();
    for nr in candidates {
        match omrade_intersects(fetch, endpoint, &nr, &rect) {
            Ok(true) => kept.push(nr),
            Ok(false) => {}
            // A candidate whose outline cannot be read is kept, not
            // dropped. This step exists to save a download, and being
            // wrong in the cheap direction costs bandwidth; being wrong
            // in the other direction costs a pack that routes through
            // water.
            Err(_) => kept.push(nr),
        }
    }
    if kept.is_empty() {
        return Err(BuildError::Logic(format!(
            "no kommune outline meets {:.4},{:.4}..{:.4},{:.4}",
            region[0], region[1], region[2], region[3]
        )));
    }

    kept.sort();
    kept.dedup();
    kept.iter().map(|n| Kommune::parse(n)).collect()
}

/// Which kommune is this point in?
///
/// `/punkt` snaps: a point at sea comes back as the nearest kommune,
/// which may be an island far outside the region. That is harmless
/// here — such a seed is discarded by the bounding-box test in step 2 —
/// and it is why the seeds cannot be trusted as an answer on their own.
fn punkt(
    fetch: &dyn Fetch,
    endpoint: &str,
    lon: f64,
    lat: f64,
) -> Result<Option<String>, BuildError> {
    let url = format!("{endpoint}/punkt?nord={lat}&ost={lon}&koordsys={CRS}");
    let r = fetch.get(&url)?;
    if !r.is_success() {
        // Not fatal. A point outside Norway has no kommune, and a
        // region on the border legitimately has some.
        return Ok(None);
    }
    let v: serde_json::Value = serde_json::from_slice(&r.body)
        .map_err(|e| BuildError::Fetch(format!("kommuneinfo /punkt: {e}")))?;
    Ok(v.get("kommunenummer")
        .and_then(|k| k.as_str())
        .map(str::to_string))
}

fn naboer(fetch: &dyn Fetch, endpoint: &str, nr: &str) -> Result<Vec<String>, BuildError> {
    let r = fetch.get(&format!("{endpoint}/kommuner/{nr}/nabokommuner"))?;
    if !r.is_success() {
        return Ok(Vec::new());
    }
    let v: serde_json::Value = serde_json::from_slice(&r.body)
        .map_err(|e| BuildError::Fetch(format!("kommuneinfo /nabokommuner: {e}")))?;
    Ok(v.as_array()
        .map(|a| {
            a.iter()
                .filter_map(|k| k.get("kommunenummer")?.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default())
}

/// The kommune's `avgrensningsboks`, as a rectangle.
fn kommune_bbox(
    fetch: &dyn Fetch,
    endpoint: &str,
    nr: &str,
) -> Result<Option<geo::Rect<f64>>, BuildError> {
    let r = fetch.get(&format!("{endpoint}/kommuner/{nr}"))?;
    if !r.is_success() {
        return Ok(None);
    }
    let v: serde_json::Value = serde_json::from_slice(&r.body)
        .map_err(|e| BuildError::Fetch(format!("kommuneinfo /kommuner/{nr}: {e}")))?;
    let coords = v
        .get("avgrensningsboks")
        .and_then(|b| b.get("coordinates"))
        .and_then(|c| c.as_array())
        .and_then(|c| c.first())
        .and_then(|c| c.as_array());
    let Some(ring) = coords else {
        return Ok(None);
    };
    let pts: Vec<(f64, f64)> = ring
        .iter()
        .filter_map(|p| {
            let a = p.as_array()?;
            Some((a.first()?.as_f64()?, a.get(1)?.as_f64()?))
        })
        .collect();
    if pts.is_empty() {
        return Ok(None);
    }
    let (mut x0, mut y0) = (f64::MAX, f64::MAX);
    let (mut x1, mut y1) = (f64::MIN, f64::MIN);
    for (x, y) in pts {
        x0 = x0.min(x);
        y0 = y0.min(y);
        x1 = x1.max(x);
        y1 = y1.max(y);
    }
    Ok(Some(geo::Rect::new(
        geo::coord! { x: x0, y: y0 },
        geo::coord! { x: x1, y: y1 },
    )))
}

/// Does the kommune's real outline meet the region?
fn omrade_intersects(
    fetch: &dyn Fetch,
    endpoint: &str,
    nr: &str,
    rect: &geo::Rect<f64>,
) -> Result<bool, BuildError> {
    let r = fetch.get(&format!("{endpoint}/kommuner/{nr}/omrade"))?;
    if !r.is_success() {
        return Err(BuildError::Fetch(format!(
            "kommuneinfo /omrade {nr}: HTTP {}",
            r.status
        )));
    }
    let v: serde_json::Value = serde_json::from_slice(&r.body)
        .map_err(|e| BuildError::Fetch(format!("kommuneinfo /omrade {nr}: {e}")))?;
    let geom = v.get("omrade").unwrap_or(&v);
    let polys = geojson_polygons(geom);
    if polys.is_empty() {
        return Err(BuildError::Fetch(format!(
            "kommuneinfo /omrade {nr}: no polygons"
        )));
    }
    // `intersects` alone would miss the case where the region sits
    // wholly inside the kommune — no boundary is crossed, so nothing
    // "intersects" in the segment sense. A small box in the middle of a
    // big kommune is the ordinary case on a phone, not an edge case.
    Ok(polys
        .iter()
        .any(|p| p.intersects(rect) || p.contains(&rect.center())))
}

/// Pull `Polygon` / `MultiPolygon` coordinates out of a GeoJSON value.
fn geojson_polygons(v: &serde_json::Value) -> Vec<geo::Polygon<f64>> {
    let Some(kind) = v.get("type").and_then(|t| t.as_str()) else {
        return Vec::new();
    };
    let Some(coords) = v.get("coordinates").and_then(|c| c.as_array()) else {
        return Vec::new();
    };
    match kind {
        "Polygon" => one_polygon(coords).into_iter().collect(),
        "MultiPolygon" => coords
            .iter()
            .filter_map(|p| one_polygon(p.as_array()?))
            .collect(),
        _ => Vec::new(),
    }
}

/// One GeoJSON polygon: an outer ring followed by holes.
///
/// The holes are kept. A kommune with an enclave inside it — Norway has
/// them — would otherwise test as covering ground it does not own.
fn one_polygon(rings: &[serde_json::Value]) -> Option<geo::Polygon<f64>> {
    let mut it = rings.iter().filter_map(|r| ring(r.as_array()?));
    let outer = it.next()?;
    Some(geo::Polygon::new(outer, it.collect()))
}

fn ring(pts: &[serde_json::Value]) -> Option<geo::LineString<f64>> {
    let cs: Vec<geo::Coord<f64>> = pts
        .iter()
        .filter_map(|p| {
            let a = p.as_array()?;
            Some(geo::coord! { x: a.first()?.as_f64()?, y: a.get(1)?.as_f64()? })
        })
        .collect();
    // Three points is the fewest that can bound an area; fewer is a
    // degenerate ring that `geo` would happily accept and then answer
    // nonsense about.
    (cs.len() >= 3).then(|| geo::LineString::new(cs))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fetch::Response;
    use std::collections::HashMap;
    use std::sync::Mutex;

    /// A Kommuneinfo stand-in built from canned routes.
    struct Api {
        routes: HashMap<String, String>,
        calls: Mutex<Vec<String>>,
    }

    impl Api {
        fn new(routes: &[(&str, &str)]) -> Self {
            Self {
                routes: routes
                    .iter()
                    .map(|(k, v)| (k.to_string(), v.to_string()))
                    .collect(),
                calls: Mutex::new(Vec::new()),
            }
        }
        fn hits(&self, needle: &str) -> usize {
            self.calls
                .lock()
                .unwrap()
                .iter()
                .filter(|u| u.contains(needle))
                .count()
        }
    }

    impl Fetch for Api {
        fn get(&self, url: &str) -> Result<Response, BuildError> {
            self.calls.lock().unwrap().push(url.to_string());
            // `/punkt` carries a query string; match on the path only.
            let path = url.split('?').next().unwrap();
            let key = if url.contains("/punkt") {
                "/punkt"
            } else {
                path
            };
            match self.routes.get(key) {
                Some(body) => Ok(Response {
                    status: 200,
                    body: body.as_bytes().to_vec(),
                }),
                None => Ok(Response {
                    status: 404,
                    body: b"{}".to_vec(),
                }),
            }
        }
        fn post_json(&self, _: &str, _: &str) -> Result<Response, BuildError> {
            unreachable!("the resolver never POSTs")
        }
    }

    /// A square kommune covering `[x0,y0]..[x1,y1]`, as the API returns it.
    fn square(x0: f64, y0: f64, x1: f64, y1: f64) -> String {
        format!(
            r#"{{"omrade":{{"type":"Polygon","coordinates":[[[{x0},{y0}],[{x1},{y0}],[{x1},{y1}],[{x0},{y1}],[{x0},{y0}]]]}}}}"#
        )
    }

    fn bbox(x0: f64, y0: f64, x1: f64, y1: f64) -> String {
        format!(
            r#"{{"avgrensningsboks":{{"type":"Polygon","coordinates":[[[{x0},{y0}],[{x0},{y1}],[{x1},{y1}],[{x1},{y0}],[{x0},{y0}]]]}}}}"#
        )
    }

    /// The ordinary case: a small region wholly inside one kommune.
    ///
    /// Nothing intersects in the segment sense here — the region does
    /// not touch a single boundary — so this is exactly the case a
    /// naive `intersects` check gets wrong.
    #[test]
    fn a_region_inside_one_kommune_resolves_to_it() {
        let api = Api::new(&[
            ("/punkt", r#"{"kommunenummer":"1845"}"#),
            ("api/kommuner/1845", &bbox(15.0, 67.0, 16.0, 68.0)),
            ("api/kommuner/1845/nabokommuner", "[]"),
            ("api/kommuner/1845/omrade", &square(15.0, 67.0, 16.0, 68.0)),
        ]);
        let got = resolve(&api, [15.4, 67.4, 15.5, 67.5], "api").unwrap();
        assert_eq!(
            got.iter().map(|k| k.0.as_str()).collect::<Vec<_>>(),
            ["1845"]
        );
    }

    /// A region straddling two kommuner must fetch both.
    #[test]
    fn a_straddling_region_resolves_to_both() {
        let api = Api::new(&[
            ("/punkt", r#"{"kommunenummer":"1845"}"#),
            ("api/kommuner/1845", &bbox(15.0, 67.0, 15.5, 68.0)),
            (
                "api/kommuner/1845/nabokommuner",
                r#"[{"kommunenummer":"1841"}]"#,
            ),
            ("api/kommuner/1845/omrade", &square(15.0, 67.0, 15.5, 68.0)),
            ("api/kommuner/1841", &bbox(15.5, 67.0, 16.0, 68.0)),
            ("api/kommuner/1841/nabokommuner", "[]"),
            ("api/kommuner/1841/omrade", &square(15.5, 67.0, 16.0, 68.0)),
        ]);
        let got = resolve(&api, [15.4, 67.4, 15.6, 67.5], "api").unwrap();
        assert_eq!(
            got.iter().map(|k| k.0.as_str()).collect::<Vec<_>>(),
            ["1841", "1845"]
        );
    }

    /// A neighbour whose bounding box misses the region is never
    /// expanded — this is what stops the walk from reaching Oslo.
    #[test]
    fn a_far_neighbour_is_not_expanded() {
        let api = Api::new(&[
            ("/punkt", r#"{"kommunenummer":"1845"}"#),
            ("api/kommuner/1845", &bbox(15.0, 67.0, 16.0, 68.0)),
            (
                "api/kommuner/1845/nabokommuner",
                r#"[{"kommunenummer":"0301"}]"#,
            ),
            ("api/kommuner/1845/omrade", &square(15.0, 67.0, 16.0, 68.0)),
            ("api/kommuner/0301", &bbox(10.0, 59.0, 11.0, 60.0)),
            (
                "api/kommuner/0301/nabokommuner",
                r#"[{"kommunenummer":"3205"}]"#,
            ),
        ]);
        let got = resolve(&api, [15.4, 67.4, 15.5, 67.5], "api").unwrap();
        assert_eq!(
            got.iter().map(|k| k.0.as_str()).collect::<Vec<_>>(),
            ["1845"]
        );
        assert_eq!(api.hits("/kommuner/0301/omrade"), 0, "never fetched");
        assert_eq!(api.hits("/kommuner/3205"), 0, "never reached");
    }

    /// The narrowing step: a kommune whose box meets the region but
    /// whose outline does not is dropped, saving a 25 MB N50 order.
    #[test]
    fn a_box_that_meets_but_an_outline_that_does_not_is_dropped() {
        let api = Api::new(&[
            ("/punkt", r#"{"kommunenummer":"1845"}"#),
            ("api/kommuner/1845", &bbox(15.0, 67.0, 16.0, 68.0)),
            (
                "api/kommuner/1845/nabokommuner",
                r#"[{"kommunenummer":"1841"}]"#,
            ),
            ("api/kommuner/1845/omrade", &square(15.0, 67.0, 16.0, 68.0)),
            // Box spans the region; the actual land is off to one side,
            // the shape of a kommune wrapped around a fjord.
            ("api/kommuner/1841", &bbox(15.0, 67.0, 16.0, 68.0)),
            ("api/kommuner/1841/nabokommuner", "[]"),
            ("api/kommuner/1841/omrade", &square(15.9, 67.9, 16.0, 68.0)),
        ]);
        let got = resolve(&api, [15.4, 67.4, 15.5, 67.5], "api").unwrap();
        assert_eq!(
            got.iter().map(|k| k.0.as_str()).collect::<Vec<_>>(),
            ["1845"]
        );
    }

    /// An unreadable outline keeps the kommune. Wrong in the direction
    /// that costs bandwidth, not the one that costs a wrong pack.
    #[test]
    fn an_unreadable_outline_keeps_the_kommune() {
        let api = Api::new(&[
            ("/punkt", r#"{"kommunenummer":"1845"}"#),
            ("api/kommuner/1845", &bbox(15.0, 67.0, 16.0, 68.0)),
            ("api/kommuner/1845/nabokommuner", "[]"),
            // No /omrade route — the stub answers 404.
        ]);
        let got = resolve(&api, [15.4, 67.4, 15.5, 67.5], "api").unwrap();
        assert_eq!(
            got.iter().map(|k| k.0.as_str()).collect::<Vec<_>>(),
            ["1845"]
        );
    }

    /// Outside Norway there is nothing to resolve, and saying so beats
    /// a build that starts and fails minutes later on empty N50.
    #[test]
    fn a_region_with_no_kommune_is_refused() {
        let api = Api::new(&[("/punkt", r#"{}"#)]);
        let e = resolve(&api, [2.0, 48.0, 2.1, 48.1], "api").unwrap_err();
        assert!(format!("{e}").contains("outside Norway"), "{e}");
    }
}
