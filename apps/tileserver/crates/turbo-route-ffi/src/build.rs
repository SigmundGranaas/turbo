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
/// **Pass an empty list** to have them resolved from `bounds`, which is
/// what a host driving this from a map selection should do.
///
/// Supplying them by hand is for callers that already know. Getting the
/// list wrong does not fail the build: it writes a pack with a water
/// mask that stops at an invisible line and roads that end there, which
/// looks entirely healthy and routes through lakes.
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
    let parsed: Result<Vec<_>, _> = kommuner
        .iter()
        .map(|k| turbo_pack_build::n50::Kommune::parse(k))
        .collect();
    let parsed = parsed.map_err(|e| RouteError::InvalidRequest(e.to_string()))?;

    let cancelled = Arc::new(AtomicBool::new(false));
    let flag = cancelled.clone();

    let fetch = HostFetch(http);

    // An empty list is not an error, it is the normal case. A phone
    // user drags a box on a map; nobody expects them to know that N50
    // is ordered per kommune, let alone which of the 357 their box
    // covers. Resolving it here rather than in the host keeps the one
    // piece of logic that must not be wrong — too few kommuner is a
    // pack with a hole in its water mask — in one tested place instead
    // of one per platform.
    let parsed = if parsed.is_empty() {
        progress.on_progress(BuildPhase::Vectors, 0, 1);
        turbo_pack_build::kommune::resolve(
            &fetch,
            [
                bounds.min_lon,
                bounds.min_lat,
                bounds.max_lon,
                bounds.max_lat,
            ],
            turbo_pack_build::kommune::DEFAULT_ENDPOINT,
        )
        .map_err(|e| RouteError::Pack(e.to_string()))?
    } else {
        parsed
    };
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
                    // Already cancelled. Keep saying so — the build
                    // reads this answer to decide whether to stop, and
                    // returning true here would restart a build the user
                    // has already dismissed.
                    return false;
                }
                let p = match phase {
                    turbo_pack_build::region::Phase::Dem => BuildPhase::Terrain,
                    turbo_pack_build::region::Phase::Vector => BuildPhase::Vectors,
                    turbo_pack_build::region::Phase::Mask => BuildPhase::Water,
                    turbo_pack_build::region::Phase::Graph => BuildPhase::Trails,
                    turbo_pack_build::region::Phase::Manifest => BuildPhase::Finishing,
                };
                let keep_going = progress.on_progress(p, done as u32, total as u32);
                if !keep_going {
                    flag.store(true, Ordering::Relaxed);
                }
                keep_going
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

    /// An HTTP callback that records what it was asked for and refuses.
    ///
    /// Refusing rather than answering keeps these tests off the network
    /// while still letting them assert *whether* it would have been
    /// reached, and for what.
    #[derive(Default)]
    struct SpyHttp {
        urls: std::sync::Mutex<Vec<String>>,
    }
    impl PackHttp for SpyHttp {
        fn get(&self, url: String) -> Result<HttpResponse, RouteError> {
            self.urls.lock().unwrap().push(url);
            Err(RouteError::Pack("offline".into()))
        }
        fn post_json(&self, url: String, _: String) -> Result<HttpResponse, RouteError> {
            self.urls.lock().unwrap().push(url);
            Err(RouteError::Pack("offline".into()))
        }
    }

    /// An empty list is the instruction to resolve the kommuner from
    /// the bounds — the normal case for a host driving this from a map
    /// selection, where nobody knows N50 is ordered per kommune.
    ///
    /// Asserted through the URL it reaches for, because the alternative
    /// reading — "empty means no N50" — also produces no pack, just
    /// silently and for the wrong reason.
    #[test]
    fn an_empty_kommune_list_resolves_from_the_bounds() {
        let http = Arc::new(SpyHttp::default());
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
            http.clone(),
            Arc::new(Never),
        )
        .unwrap_err();
        // The resolver ran and the offline stub stopped it there.
        assert!(matches!(e, RouteError::Pack(_)), "{e:?}");
        let urls = http.urls.lock().unwrap();
        assert!(
            urls.iter().any(|u| u.contains("kommuneinfo")),
            "expected a kommune lookup, got {urls:?}"
        );
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
            Arc::new(SpyHttp::default()),
            Arc::new(Never),
        )
        .unwrap_err();
        assert!(matches!(e, RouteError::InvalidRequest(_)), "{e:?}");
        assert!(!std::path::Path::new("/tmp/never2").exists());
    }
}
