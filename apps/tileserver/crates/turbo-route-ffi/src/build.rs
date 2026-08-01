//! Building a pack on the device, across the FFI.
//!
//! The counterpart to downloading one. `PackDownloader` on the Android
//! side fetches a pack somebody else cut; this cuts one from
//! Kartverket's public services directly, so a region nobody has
//! prepared is still routable.
//!
//! # Shape
//!
//! Blocking with a progress callback, matching
//! [`crate::RouteEngine::plan_with_progress`] rather than inventing a
//! second async convention. The host calls it off its main thread — it
//! is minutes of network — and cancels by returning `false` from
//! [`PackBuildProgress::on_progress`], which is the same mechanism the
//! solver's observer uses inverted: there, a host that wants to stop
//! simply stops asking; here it must be able to say so mid-fetch.
//!
//! # The host does the fetching
//!
//! This library carries no HTTP client. [`PackHttp`] is a callback the
//! host implements — on Android with the OkHttp instance the app
//! already has, complete with its connection pool, retries, proxy and
//! certificate handling.
//!
//! That is not tidiness, it is size. Linking `reqwest` + `rustls` +
//! `tokio` in here measured **+3.2 MB per ABI**, roughly tripling the
//! library, and almost none of it was the GML and GeoTIFF parsers this
//! crate exists for — it was a second TLS implementation and a second
//! async runtime sitting next to the ones the app already ships.
//!
//! It also means one fewer place for a certificate or proxy policy to
//! disagree with the rest of the app.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use crate::RouteError;

/// One HTTP response, as the host saw it.
#[derive(Debug, Clone, uniffi::Record)]
pub struct HttpResponse {
    /// The status code. Non-2xx is reported, not thrown: the builder's
    /// diagnostics quote the status *and* the body — a WCS 400 carries
    /// an XML explanation worth surfacing — so a host that collapsed
    /// failures into an exception would lose it.
    pub status: u16,
    pub body: Vec<u8>,
}

/// How the library reaches Kartverket, supplied by the host.
///
/// Implementations must be safe to call from several threads at once:
/// the DEM fetch runs a few requests in parallel. They must also apply
/// a generous timeout — a cold WCS coverage can take minutes, and a
/// default 30-second client will fail builds that would have worked.
#[uniffi::export(with_foreign)]
pub trait PackHttp: Send + Sync {
    fn get(&self, url: String) -> Result<HttpResponse, RouteError>;
    /// POST a JSON body. Used only by the N50 order API.
    fn post_json(&self, url: String, body: String) -> Result<HttpResponse, RouteError>;
}

/// Adapts the host's callback to the builder's port.
struct HostFetch(Arc<dyn PackHttp>);

impl turbo_pack_build::fetch::Fetch for HostFetch {
    fn get(
        &self,
        url: &str,
    ) -> Result<turbo_pack_build::fetch::Response, turbo_pack_build::BuildError> {
        let r = self
            .0
            .get(url.to_string())
            .map_err(|e| turbo_pack_build::BuildError::Fetch(e.to_string()))?;
        Ok(turbo_pack_build::fetch::Response {
            status: r.status,
            body: r.body,
        })
    }

    fn post_json(
        &self,
        url: &str,
        body: &str,
    ) -> Result<turbo_pack_build::fetch::Response, turbo_pack_build::BuildError> {
        let r = self
            .0
            .post_json(url.to_string(), body.to_string())
            .map_err(|e| turbo_pack_build::BuildError::Fetch(e.to_string()))?;
        Ok(turbo_pack_build::fetch::Response {
            status: r.status,
            body: r.body,
        })
    }
}

/// Which stage a build is in, for a host that wants to label its bar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum BuildPhase {
    /// Fetching terrain. The long one — most of a pack is its DEM.
    Terrain,
    /// Ordering and parsing N50, and fetching trails.
    Vectors,
    /// Rasterising water and glaciers.
    Water,
    /// Noding and writing the trail network.
    Trails,
    /// Digests and the manifest.
    Finishing,
}

/// Progress and cancellation for a pack build.
#[uniffi::export(with_foreign)]
pub trait PackBuildProgress: Send + Sync {
    /// Called as the build advances.
    ///
    /// Return `false` to cancel. Cancellation is checked between units
    /// of work, not inside one, so a build stops after the coverage
    /// request in flight rather than tearing one in half — a partial
    /// artifact is the failure this whole pipeline is built to avoid.
    fn on_progress(&self, phase: BuildPhase, done: u32, total: u32) -> bool;
}

/// What a device build produced.
#[derive(Debug, Clone, uniffi::Record)]
pub struct PackBuildResult {
    pub dir: String,
    pub dem_tiles: u64,
    pub nodes: u32,
    pub edges: u32,
    pub trails: u32,
    pub roads: u32,
    pub refused_cells: u64,
    pub total_bytes: u64,
    pub seconds: f64,
}

/// Build a routing pack for `extent` into `out_dir`.
///
/// `kommuner` are the kommune numbers whose N50 data the region needs.
/// They are required, and the build refuses a list that does not cover
/// the region rather than writing a pack with an empty water mask and
/// no roads — which looks entirely healthy and routes through lakes.
/// The region to cut, as one value.
///
/// Named fields rather than four positional `f64`s. Across the FFI the
/// caller writes them out in order, and lon/lat transposition is both
/// the easiest mistake to make here and one of the hardest to see: a
/// transposed bbox is still a valid rectangle, just somewhere else, so
/// the build succeeds and produces a pack of the wrong place.
#[derive(Debug, Clone, Copy, uniffi::Record)]
pub struct BuildBounds {
    pub min_lon: f64,
    pub min_lat: f64,
    pub max_lon: f64,
    pub max_lat: f64,
}

#[uniffi::export]
pub fn build_pack(
    out_dir: String,
    bounds: BuildBounds,
    halo_m: f64,
    kommuner: Vec<String>,
    http: Arc<dyn PackHttp>,
    progress: Arc<dyn PackBuildProgress>,
) -> Result<PackBuildResult, RouteError> {
    if kommuner.is_empty() {
        return Err(RouteError::InvalidRequest(
            "at least one kommune number is required — N50 has no bbox service, so water, \
             glaciers and roads are ordered per kommune"
                .into(),
        ));
    }
    let parsed: Result<Vec<_>, _> = kommuner
        .iter()
        .map(|k| turbo_pack_build::n50::Kommune::parse(k))
        .collect();
    let parsed = parsed.map_err(|e| RouteError::InvalidRequest(e.to_string()))?;

    let cancelled = Arc::new(AtomicBool::new(false));
    let flag = cancelled.clone();

    let fetch = HostFetch(http);
    let out = std::path::PathBuf::from(&out_dir);
    let report = {
        turbo_pack_build::region::build_pack(
            &fetch,
            &out,
            [
                bounds.min_lon,
                bounds.min_lat,
                bounds.max_lon,
                bounds.max_lat,
            ],
            halo_m,
            &parsed,
            turbo_pack_build::wcs::DEFAULT_ENDPOINT,
            turbo_pack_build::wfs::DEFAULT_ENDPOINT,
            // Two requests in flight. Small on purpose: this is a public
            // service shared by everyone in the country, and a phone
            // opening a socket per core is the behaviour that gets an
            // app blocked.
            2,
            |phase, done, total| {
                if flag.load(Ordering::Relaxed) {
                    return;
                }
                let p = match phase {
                    turbo_pack_build::region::Phase::Dem => BuildPhase::Terrain,
                    turbo_pack_build::region::Phase::Vector => BuildPhase::Vectors,
                    turbo_pack_build::region::Phase::Mask => BuildPhase::Water,
                    turbo_pack_build::region::Phase::Graph => BuildPhase::Trails,
                    turbo_pack_build::region::Phase::Manifest => BuildPhase::Finishing,
                };
                if !progress.on_progress(p, done as u32, total as u32) {
                    flag.store(true, Ordering::Relaxed);
                }
            },
        )
    };

    if cancelled.load(Ordering::Relaxed) {
        // Leave nothing behind. A half-built pack has a DEM and no
        // graph, which opens, reports coverage, and routes cross-country
        // over ground it has no trails for.
        let _ = std::fs::remove_dir_all(&out);
        return Err(RouteError::Internal("cancelled".into()));
    }

    let r = report.map_err(|e| RouteError::Pack(e.to_string()))?;
    Ok(PackBuildResult {
        dir: out_dir,
        dem_tiles: r.dem_tiles,
        nodes: r.nodes,
        edges: r.edges_directed,
        trails: r.trails as u32,
        roads: r.roads as u32,
        refused_cells: r.refused_cells,
        total_bytes: r.total_bytes,
        seconds: r.seconds,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Never;
    impl PackBuildProgress for Never {
        fn on_progress(&self, _: BuildPhase, _: u32, _: u32) -> bool {
            true
        }
    }

    /// An HTTP callback that fails the test if it is ever reached.
    ///
    /// The point of both tests below is that the argument check happens
    /// *before* any network work, so a stub that quietly returned an
    /// empty body would let a regression through silently.
    struct NoHttp;
    impl PackHttp for NoHttp {
        fn get(&self, url: String) -> Result<HttpResponse, RouteError> {
            panic!("the network must not be touched: GET {url}");
        }
        fn post_json(&self, url: String, _: String) -> Result<HttpResponse, RouteError> {
            panic!("the network must not be touched: POST {url}");
        }
    }

    /// No kommune means no N50, and N50 is water, glaciers and roads.
    /// Building anyway would write a pack that opens and routes through
    /// lakes, so it is refused at the boundary rather than later.
    #[test]
    fn refuses_a_build_with_no_kommune() {
        let e = build_pack(
            "/tmp/never".into(),
            BuildBounds {
                min_lon: 15.0,
                min_lat: 67.0,
                max_lon: 15.1,
                max_lat: 67.1,
            },
            0.0,
            vec![],
            Arc::new(NoHttp),
            Arc::new(Never),
        )
        .unwrap_err();
        assert!(matches!(e, RouteError::InvalidRequest(_)), "{e:?}");
        assert!(!std::path::Path::new("/tmp/never").exists());
    }

    #[test]
    fn refuses_a_malformed_kommune_before_touching_the_network() {
        let e = build_pack(
            "/tmp/never2".into(),
            BuildBounds {
                min_lon: 15.0,
                min_lat: 67.0,
                max_lon: 15.1,
                max_lat: 67.1,
            },
            0.0,
            vec!["Sørfold".into()],
            Arc::new(NoHttp),
            Arc::new(Never),
        )
        .unwrap_err();
        assert!(matches!(e, RouteError::InvalidRequest(_)), "{e:?}");
        assert!(!std::path::Path::new("/tmp/never2").exists());
    }
}
