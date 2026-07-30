//! **L6 — the foreign-language façade.** What Kotlin and Swift hosts
//! call to plan a route on-device.
//!
//! See `docs/architecture/2026-07-routing-engine-module-design.md` §8.1.
//!
//! # Why this is coarse on purpose
//!
//! Everything below L6 was built to have clean seams: ports at L1,
//! adapters at L3, a solver registry, a composition root that wires
//! them. This crate exposes **none** of that. A host says "open this
//! pack, plan this route" and gets a polyline.
//!
//! That is deliberate, and it is the one place in the stack where
//! combining layers is right. Fowler's First Law of Distributed Object
//! Design — don't distribute your objects — applies literally at an FFI
//! boundary: every seam exposed here becomes a breaking change for two
//! mobile apps the moment it moves. The seams exist so *Rust* can
//! evolve; the façade exists so the phone doesn't have to care.
//!
//! # What crosses the boundary
//!
//! Geographic coordinates, in and out. The engine is planar-only (C4)
//! and knows no CRS, but a phone's GPS speaks WGS84, so this layer
//! projects — the same job the HTTP API does, for the same reason, in
//! the same one place.
//!
//! # Panics do not cross
//!
//! Every exported method wraps its work in `catch_unwind`. A Rust panic
//! unwinding into the JVM or the Objective-C runtime is undefined
//! behaviour, and in practice aborts the process — the user's hiking app
//! vanishes because a solver hit an edge case on a ridge. It becomes
//! [`RouteError::Internal`] instead.

#![forbid(unsafe_code)]

use std::panic::AssertUnwindSafe;
use std::path::PathBuf;
use std::sync::Arc;

use turbo_tiles_pathfind::{Pathfinder, Point, Prefs};

uniffi::setup_scaffolding!();

// ---- value types ----------------------------------------------------

/// A WGS84 coordinate — what a phone's location API produces.
#[derive(Debug, Clone, Copy, uniffi::Record)]
pub struct GeoPoint {
    pub lon: f64,
    pub lat: f64,
}

/// Travel mode. Mirrors the profile crate's modes rather than the
/// engine's `ModeId`, because a host picking "ski" should not have to
/// know it is index 2.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum TravelMode {
    Foot,
    Bicycle,
    Ski,
}

/// The knobs a host is expected to set. Deliberately a small subset of
/// the engine's `Prefs`: everything omitted has a calibrated default,
/// and exposing a knob across FFI is a promise to keep it working.
#[derive(Debug, Clone, uniffi::Record)]
pub struct RouteOptions {
    pub mode: TravelMode,
    /// Named trip preset ("balanced", "avoid_roads", …). Unknown names
    /// are an error naming the valid ones, not a silent fallback.
    pub preset: Option<String>,
    /// Skip the trail network entirely and route cross-country.
    pub force_off_trail: bool,
    /// Refuse a request whose endpoints span more than this, straight
    /// line, in kilometres.
    ///
    /// **This is a budget, and on a phone it is load-bearing.** The
    /// engine's own guard (`max_off_trail_km`) only bounds the
    /// cross-country mesh; a request that can reach the trail network
    /// is not bounded by anything. Writing the host tests surfaced the
    /// consequence: a route from Oslo to a Sjunkhatten pack — 850 km,
    /// one endpoint outside coverage entirely — **solved, in 83
    /// seconds**. On a server that is a slow request. On a phone it is
    /// an ANR, a flat battery, and a user who force-quits.
    ///
    /// The default is generous for the use case (a long day's walk is
    /// under 50 km) and cheap to raise deliberately.
    pub max_span_km: f64,
}

impl Default for RouteOptions {
    fn default() -> Self {
        Self {
            mode: TravelMode::Foot,
            preset: Some("balanced".to_string()),
            force_off_trail: false,
            max_span_km: 100.0,
        }
    }
}

/// A planned route. Geometry is WGS84, ready to draw on a map.
#[derive(Debug, Clone, uniffi::Record)]
pub struct Route {
    pub geometry: Vec<GeoPoint>,
    pub length_m: f64,
    /// Estimated duration in seconds, from the solver's own cost model.
    pub duration_s: f64,
    /// Positive elevation gain along the route, metres.
    pub ascent_m: f64,
    /// Metres by surface type (`sti`, `vei`, `off_trail`, …). A host can
    /// show "3.1 km on trail, 800 m off" without a second call.
    pub surface_breakdown: Vec<SurfaceSpan>,
    /// Layers that refused terrain along the corridor — the honest
    /// answer to "why did it go round that way".
    pub refused_by: Vec<String>,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct SurfaceSpan {
    pub surface: String,
    pub length_m: f64,
}

/// Coverage of the loaded pack, so a host can tell the user "you are
/// outside the downloaded area" *before* asking for a route.
#[derive(Debug, Clone, Copy, uniffi::Record)]
pub struct Coverage {
    pub min_lon: f64,
    pub min_lat: f64,
    pub max_lon: f64,
    pub max_lat: f64,
}

/// Errors a host must handle. Flat, because a Kotlin `when` over a
/// shallow enum is the ergonomic shape; the detail is in the message.
#[derive(Debug, thiserror::Error, uniffi::Error)]
#[uniffi(flat_error)]
pub enum RouteError {
    /// The pack directory is missing, unreadable, or has no DEM.
    #[error("pack: {0}")]
    Pack(String),
    /// Both endpoints are outside the pack's coverage. Distinct from
    /// `NoRoute` because the fix is different — download more, rather
    /// than pick a different line.
    #[error("outside the downloaded area: {0}")]
    OutsideCoverage(String),
    /// An endpoint is somewhere untraversable — the classic case is a
    /// tap that lands in a lake.
    #[error("endpoint blocked: {0}")]
    EndpointBlocked(String),
    /// No route exists through the terrain. An honest answer, not a
    /// fallback straight line.
    #[error("no route found")]
    NoRoute,
    #[error("invalid request: {0}")]
    InvalidRequest(String),
    /// The request exceeded [`RouteOptions::max_span_km`]. Its own
    /// variant because the host's response is specific — tell the user
    /// the route is too long, rather than that it failed.
    #[error("route too long: {0}")]
    TooLong(String),
    /// A panic was caught, or the engine failed in a way the host
    /// cannot act on.
    #[error("internal: {0}")]
    Internal(String),
}

// ---- the engine handle ----------------------------------------------

/// A routing engine bound to one offline pack.
///
/// Thread-safe and cheap to share — uniffi hands hosts an `Arc`. Hold
/// one for the lifetime of the app: construction bulk-loads the trail
/// R-trees, which E4 measured at 555 ms on a national graph, and
/// rebuilding it per request is the exact mistake this architecture was
/// reorganised to make impossible.
#[derive(uniffi::Object)]
pub struct RouteEngine {
    pathfinder: Pathfinder,
    coverage: Coverage,
}

#[uniffi::export]
impl RouteEngine {
    /// Open an offline pack — a directory of `norway.{dem,mask,graph,
    /// graph_geom}` as produced by `tileserver slice-pack`.
    ///
    /// The DEM is required; everything else degrades. Without a graph
    /// there is no trail network and routes are cross-country; without a
    /// mask, water and glaciers are not refused. A missing DEM is fatal
    /// because terrain is what the engine reasons about — routing
    /// without it would return a straight line, which the codebase
    /// elsewhere calls "semantically a lie".
    #[uniffi::constructor]
    pub fn open(pack_dir: String) -> Result<Self, RouteError> {
        guard(|| {
            let dir = PathBuf::from(&pack_dir);
            if !dir.is_dir() {
                return Err(RouteError::Pack(format!("{pack_dir} is not a directory")));
            }

            let dem_path = dir.join("norway.dem");
            let dem = turbo_tiles_elev::Dem::open(&dem_path)
                .map_err(|e| RouteError::Pack(format!("{}: {e}", dem_path.display())))?;
            let cov = dem.coverage();
            let (min_lon, min_lat) = turbo_geo_frame::utm33n_to_wgs84(cov.min_x, cov.min_y);
            let (max_lon, max_lat) = turbo_geo_frame::utm33n_to_wgs84(cov.max_x, cov.max_y);
            let field = turbo_geodata_artifacts::heightfield(Arc::new(dem));

            let mask = turbo_tiles_mask::Mask::open(dir.join("norway.mask"))
                .ok()
                .map(Arc::new);

            let graph = match turbo_tiles_graph::Graph::open(dir.join("norway.graph")) {
                Ok(mut g) => {
                    // Polylines are optional; without them routes fall
                    // back to endpoint segments, which is a fidelity
                    // loss rather than a failure.
                    let _ = g.attach_geom(dir.join("norway.graph_geom"));
                    Some(Arc::new(g))
                }
                Err(_) => None,
            };

            // The composition step, done here because a phone host has
            // no business doing it: erase the artifacts to ports, take
            // the calibrated constants from the profile crate, hand the
            // engine values.
            let pathfinder = Pathfinder::with_defaults(
                Some(field),
                mask,
                graph,
                turbo_profile_no::cost_config()
                    .map_err(|e| RouteError::Internal(format!("profile: {e}")))?,
            );

            Ok(Self {
                pathfinder,
                coverage: Coverage {
                    min_lon,
                    min_lat,
                    max_lon,
                    max_lat,
                },
            })
        })
    }

    /// The pack's bounding box, for "you are outside the downloaded
    /// area" *before* a failed route rather than after one.
    ///
    /// A bounding box, not the coverage set: the DEM stores sparse
    /// tiles, so a point inside this box may still have no data. Use it
    /// to gate the UI, not to promise a route.
    pub fn coverage(&self) -> Coverage {
        self.coverage
    }

    /// Plan a route through an ordered list of points (start, any vias,
    /// end).
    pub fn plan(&self, points: Vec<GeoPoint>, options: RouteOptions) -> Result<Route, RouteError> {
        guard(|| {
            if points.len() < 2 {
                return Err(RouteError::InvalidRequest(
                    "a route needs at least a start and an end".into(),
                ));
            }

            let mut prefs = Prefs {
                profile: match options.mode {
                    TravelMode::Foot => turbo_tiles_graph::Profile::Foot,
                    TravelMode::Bicycle => turbo_tiles_graph::Profile::Bicycle,
                    TravelMode::Ski => turbo_tiles_graph::Profile::Ski,
                },
                force_off_trail: options.force_off_trail,
                ..Default::default()
            };
            if let Some(name) = options.preset.as_deref() {
                let presets = turbo_profile_no::presets();
                let p = presets.get(name).ok_or_else(|| {
                    let valid: Vec<&str> =
                        presets.presets.iter().map(|p| p.name.as_str()).collect();
                    RouteError::InvalidRequest(format!(
                        "unknown preset '{name}'; valid: {}",
                        valid.join(", ")
                    ))
                })?;
                prefs.cost_config_override = Some(p.patch.clone());
            }

            // Geographic in, planar through the engine, geographic out.
            let planar: Vec<Point> = points
                .iter()
                .map(|p| turbo_geo_frame::wgs84_to_utm33n(p.lon, p.lat))
                .collect();

            // Budget check, before any solving. Cheap, and the thing it
            // prevents is measured: see `RouteOptions::max_span_km`.
            let span_m: f64 = planar
                .windows(2)
                .map(|w| {
                    let (dx, dy) = (w[1].x - w[0].x, w[1].y - w[0].y);
                    (dx * dx + dy * dy).sqrt()
                })
                .sum();
            if options.max_span_km > 0.0 && span_m > options.max_span_km * 1000.0 {
                return Err(RouteError::TooLong(format!(
                    "{:.0} km end to end exceeds the {:.0} km limit",
                    span_m / 1000.0,
                    options.max_span_km
                )));
            }

            let path = self
                .pathfinder
                .solve_route(&planar, prefs)
                .map_err(map_err)?;

            let geometry: Vec<GeoPoint> = path
                .geometry
                .iter()
                .map(|p| {
                    let (lon, lat) = turbo_geo_frame::utm33n_to_wgs84(p.x, p.y);
                    GeoPoint { lon, lat }
                })
                .collect();

            let ascent_m = ascent_along(&self.pathfinder, &path.geometry);
            let mut surface_breakdown: Vec<SurfaceSpan> = path
                .fkb_breakdown
                .iter()
                .map(|(surface, length_m)| SurfaceSpan {
                    surface: surface.clone(),
                    length_m: *length_m,
                })
                .collect();
            // BTreeMap iteration is already ordered by name; sort by
            // length so a host can render "mostly trail" without
            // sorting it again.
            surface_breakdown.sort_by(|a, b| b.length_m.total_cmp(&a.length_m));

            Ok(Route {
                geometry,
                length_m: path.length_m,
                duration_s: path.cost,
                ascent_m,
                surface_breakdown,
                refused_by: path.refused_by.clone(),
            })
        })
    }

    /// Is there terrain data at this point? Cheap — one DEM lookup.
    ///
    /// Exposed because "tap to set a waypoint" wants an answer before
    /// the user commits, and a failed route is a bad way to find out.
    pub fn has_coverage(&self, point: GeoPoint) -> bool {
        let p = turbo_geo_frame::wgs84_to_utm33n(point.lon, point.lat);
        std::panic::catch_unwind(AssertUnwindSafe(|| self.pathfinder.point_covered(p.x, p.y)))
            .unwrap_or(false)
    }
}

// ---- plumbing -------------------------------------------------------

/// Run `f`, converting a panic into [`RouteError::Internal`].
///
/// Unwinding into the JVM or the Objective-C runtime is undefined
/// behaviour and in practice aborts the process — the user's hiking app
/// disappears because the solver hit an edge case on a ridge. A caught
/// panic is a bug either way, but one of them leaves a route request
/// failed and the other leaves the phone with no app.
fn guard<T>(f: impl FnOnce() -> Result<T, RouteError>) -> Result<T, RouteError> {
    match std::panic::catch_unwind(AssertUnwindSafe(f)) {
        Ok(r) => r,
        Err(p) => {
            let msg = p
                .downcast_ref::<&str>()
                .map(|s| (*s).to_string())
                .or_else(|| p.downcast_ref::<String>().cloned())
                .unwrap_or_else(|| "panic with no message".to_string());
            Err(RouteError::Internal(format!("caught panic: {msg}")))
        }
    }
}

/// Map the engine's error to the host's.
///
/// The mapping is not mechanical: `NoCoverage` and `EndpointRefused`
/// become distinct variants because the *user's* fix differs — download
/// more of the map, versus move the pin off the lake. Collapsing them
/// into one "no route" is what makes an app feel broken.
fn map_err(e: turbo_tiles_pathfind::PathfindError) -> RouteError {
    use turbo_tiles_pathfind::PathfindError as E;
    match e {
        E::NoCoverage { .. } => RouteError::OutsideCoverage(e.to_string()),
        E::EndpointRefused { .. } => RouteError::EndpointBlocked(e.to_string()),
        E::NoRoute => RouteError::NoRoute,
        E::DegenerateInputs { .. } => RouteError::InvalidRequest(e.to_string()),
        other => RouteError::Internal(other.to_string()),
    }
}

/// Positive elevation gain along a planar polyline.
fn ascent_along(pf: &Pathfinder, geom: &[Point]) -> f64 {
    let Some(dem) = pf.dem.as_ref() else {
        return 0.0;
    };
    let mut gain = 0.0;
    let mut prev: Option<f32> = None;
    for p in geom {
        let Some(z) = dem.height_at(*p) else { continue };
        if let Some(a) = prev {
            if z > a {
                gain += (z - a) as f64;
            }
        }
        prev = Some(z);
    }
    gain
}

/// Route Rust logs to logcat so a device build is debuggable at all.
/// No-op elsewhere.
#[cfg(target_os = "android")]
#[uniffi::export]
pub fn init_logging() {
    android_logger::init_once(
        android_logger::Config::default().with_max_level(log::LevelFilter::Info),
    );
}

#[cfg(not(target_os = "android"))]
#[uniffi::export]
pub fn init_logging() {}
