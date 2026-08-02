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
// The solver recorder holds a `RefCell`, so it is `Send` but not `Sync`,
// and `with_installed` takes it as an `Arc` because that is the shape the
// thread-local install uses — never crossing a thread. The engine and the
// HTTP crate carry the same allow for the same `Arc`, for the same reason.
#![allow(clippy::arc_with_non_send_sync)]

use std::panic::AssertUnwindSafe;
use std::path::PathBuf;
use std::sync::Arc;

use turbo_tiles_pathfind::{Pathfinder, Point, Prefs};

uniffi::setup_scaffolding!();

pub mod build;

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
    #[uniffi(default = Some("balanced"))]
    pub preset: Option<String>,
    /// Skip the trail network entirely and route cross-country.
    #[uniffi(default = false)]
    pub force_off_trail: bool,
    /// Close the route back to its origin, taking a different line home.
    ///
    /// The engine solves the outbound leg, feeds that geometry back as
    /// an avoided polyline, and solves the return — so the loop diverges
    /// where the network allows and gracefully degrades to an
    /// out-and-back on a single-path spur.
    ///
    /// Exposed because the Android app already sends it on every request
    /// (`RouteViewModel.roundTrip`). Without it here, the on-device path
    /// would answer a round-trip request with a one-way route: the worst
    /// kind of gap, because it looks like it worked.
    #[uniffi(default = false)]
    pub round_trip: bool,
    /// Polylines to route around, WGS84. The penalty lands on the trail
    /// edges the geometry runs along, so the router detours onto a
    /// divergent trail rather than shadow-walking beside the avoided one.
    ///
    /// Soft, not a veto: an avoided path with no alternative is still
    /// used, expensively. "Avoid" is a preference, and a router that
    /// turns it into a refusal strands the user.
    #[uniffi(default = [])]
    pub avoid: Vec<Vec<GeoPoint>>,
    /// How far (m) from an avoided polyline a trail edge is still
    /// considered part of it. `None` uses the profile's calibrated value.
    #[uniffi(default = None)]
    pub avoid_radius_m: Option<f64>,
    /// Refuse a request whose endpoints span more than this, straight
    /// line, in kilometres.
    ///
    /// **This is a budget, and on a phone it is load-bearing.** Writing
    /// the host tests surfaced why: a route from Oslo to a Sjunkhatten
    /// pack — 850 km, one endpoint outside coverage entirely — **solved,
    /// in 83 seconds**. On a server that is a slow request. On a phone it
    /// is an ANR, a flat battery, and a user who force-quits.
    ///
    /// This used to say the engine's `max_off_trail_km` bounded the
    /// cross-country mesh and that only the trail-network case was
    /// unbounded. That was wrong in the way documentation goes wrong:
    /// the knob existed, was defaulted, was hashed into the leg
    /// fingerprint and echoed by the debug endpoint, and **nothing read
    /// it**. Nothing was bounded, which is why 850 km solved at all. It
    /// is enforced now (`solvers.rs`, `FmmGradeLimited::solve`), and
    /// [`Self::max_off_trail_km`] below sets it per request.
    ///
    /// The default is generous for the use case (a long day's walk is
    /// under 50 km) and cheap to raise deliberately.
    #[uniffi(default = 100.0)]
    pub max_span_km: f64,
    /// Span budget for the **cross-country lane only**, in kilometres.
    ///
    /// Separate from [`Self::max_span_km`] because the two lanes cost
    /// wildly different amounts, and one budget for both is either too
    /// tight for trail routing or too loose for terrain. Measured
    /// through this façade on the CI pack, release build, desktop:
    ///
    /// ```text
    ///            unified    cross-country
    ///   1 km      251 ms         427 ms
    ///   4 km      249 ms         734 ms
    ///   6 km      293 ms       1 891 ms
    ///  11 km      265 ms       8 515 ms
    /// ```
    ///
    /// The unified lane is flat in distance — adaptive cell sizing
    /// bounds its work. The cross-country lane is not. A phone core is
    /// slower than that desktop, so the right default here is well below
    /// the whole-request budget.
    #[uniffi(default = 10.0)]
    pub max_off_trail_km: f64,
}

impl Default for RouteOptions {
    fn default() -> Self {
        Self {
            mode: TravelMode::Foot,
            preset: Some("balanced".to_string()),
            force_off_trail: false,
            round_trip: false,
            avoid: Vec::new(),
            avoid_radius_m: None,
            max_span_km: 100.0,
            // The engine's own default, restated rather than inherited:
            // a host reading this file should see the number it gets.
            max_off_trail_km: 10.0,
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

/// Receives the best path found so far, while a solve is running.
///
/// A callback interface rather than a returned stream, because uniffi's
/// blocking call shape is the one every host already uses and this rides
/// inside it — no async runtime on either side, no second API to cancel.
#[uniffi::export(with_foreign)]
pub trait RouteProgress: Send + Sync {
    /// A better path than the last one, WGS84, ready to draw.
    ///
    /// Called many times per solve and cheap to ignore: a host that only
    /// wants the final route uses [`RouteEngine::plan`] instead.
    fn on_progress(&self, geometry: Vec<GeoPoint>);
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

            // The manifest first, before any bytes are mapped. A pack
            // written by a newer build may mean something this one would
            // misread, and finding that out after routing is worse than
            // finding it out at open: on a phone the pack sits on the
            // user's disk until they delete it, so a silently-wrong route
            // is a permanent condition rather than a bad deploy.
            //
            // Absent is fine. Packs cut before the manifest existed —
            // including the committed CI pack — still open, because the
            // artifacts have always been enough to route. The manifest
            // adds refusal and a cheap extent, not permission.
            let manifest_path = dir.join(turbo_tiles_artifacts::PackManifest::FILENAME);
            let manifest: Option<turbo_tiles_artifacts::PackManifest> =
                match std::fs::read_to_string(&manifest_path) {
                    Ok(text) => {
                        let m: turbo_tiles_artifacts::PackManifest = toml::from_str(&text)
                            .map_err(|e| {
                                RouteError::Pack(format!("{}: {e}", manifest_path.display()))
                            })?;
                        m.check_compatible()
                            .map_err(|e| RouteError::Pack(e.to_string()))?;
                        Some(m)
                    }
                    Err(_) => None,
                };

            let dem_path = dir.join("norway.dem");
            let dem = turbo_tiles_elev::Dem::open(&dem_path)
                .map_err(|e| RouteError::Pack(format!("{}: {e}", dem_path.display())))?;
            // Coverage: the manifest's extent when there is one, else the
            // DEM's own bounds.
            //
            // They are not the same claim. The DEM's bounds are whole
            // tiles plus the halo — ground the pack HAS, deliberately
            // wider than the ground it was cut to serve, so that routing
            // inside the region can see far enough. The manifest records
            // what was requested. A host gating "can I route here?" wants
            // the second: offering the margin invites routes whose
            // quality the halo exists to admit is worse at the edge.
            let cov = dem.coverage();
            let (min_lon, min_lat, max_lon, max_lat) = match &manifest {
                Some(m) => (
                    m.pack.extent[0],
                    m.pack.extent[1],
                    m.pack.extent[2],
                    m.pack.extent[3],
                ),
                None => {
                    let (min_lon, min_lat) = turbo_geo_frame::utm33n_to_wgs84(cov.min_x, cov.min_y);
                    let (max_lon, max_lat) = turbo_geo_frame::utm33n_to_wgs84(cov.max_x, cov.max_y);
                    (min_lon, min_lat, max_lon, max_lat)
                }
            };
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
        self.plan_inner(points, options, None)
    }

    /// As [`Self::plan`], reporting the best path found so far as the
    /// solver works.
    ///
    /// This is what lets an on-device route **draw** rather than appear.
    /// The server has streamed these snapshots for a while — its SSE
    /// endpoint reads them off the same solver hook — and their absence
    /// here was the one visible way the phone was worse than the network.
    ///
    /// `observer` is called on the calling thread, synchronously, before
    /// `plan_with_progress` returns. A host that hops to its UI thread
    /// inside the callback must not block waiting for it: the solver is
    /// stopped for the duration, and a round trip per snapshot would
    /// dominate the solve.
    pub fn plan_with_progress(
        &self,
        points: Vec<GeoPoint>,
        options: RouteOptions,
        observer: Arc<dyn RouteProgress>,
    ) -> Result<Route, RouteError> {
        self.plan_inner(points, options, Some(observer))
    }

    fn plan_inner(
        &self,
        points: Vec<GeoPoint>,
        options: RouteOptions,
        observer: Option<Arc<dyn RouteProgress>>,
    ) -> Result<Route, RouteError> {
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
                round_trip: options.round_trip,
                avoid_radius_m: options.avoid_radius_m,
                max_off_trail_km: options.max_off_trail_km,
                avoid: options
                    .avoid
                    .iter()
                    .map(|ring| {
                        ring.iter()
                            .map(|p| turbo_geo_frame::wgs84_to_utm33n(p.lon, p.lat))
                            .collect()
                    })
                    .collect(),
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

            let solve = || self.pathfinder.solve_route(&planar, prefs.clone());
            let path = match &observer {
                None => solve(),
                Some(obs) => {
                    // Snapshots are planar; the host speaks WGS84, so they
                    // are projected here — the same one place `plan`'s
                    // result is projected, for the same reason.
                    let obs = obs.clone();
                    let recorder = Arc::new(turbo_tiles_pathfind::Recorder::new_sink(Box::new(
                        move |ev| {
                            if let turbo_tiles_pathfind::SolverEvent::BestPathSnapshot { coords } =
                                ev
                            {
                                let geometry = coords
                                    .iter()
                                    .map(|c| {
                                        let (lon, lat) = turbo_geo_frame::utm33n_to_wgs84(
                                            c[0] as f64,
                                            c[1] as f64,
                                        );
                                        GeoPoint { lon, lat }
                                    })
                                    .collect();
                                obs.on_progress(geometry);
                            }
                        },
                    )));
                    turbo_tiles_pathfind::solver_trace::with_installed(recorder, solve)
                }
            }
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
        // The cross-country budget, refused. `TooLong` rather than
        // `Internal` because the host's response is specific and the
        // user's fix is real: shorten the leg, or raise
        // `max_off_trail_km` deliberately.
        E::BboxTooLarge { .. } => RouteError::TooLong(e.to_string()),
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
///
/// A no-op off Android, and a no-op with the `logcat` feature disabled —
/// which a statically-linked build must do, since `liblog` exists only as
/// a shared library. The export stays either way: the bindings are
/// generated once, and a host should not have to know how this build was
/// configured to know whether the function is there.
///
/// # The `cfg` is inside the body, and has to stay there
///
/// This was two `#[uniffi::export]`s under opposite `cfg`s, and it
/// shipped a broken app. uniffi hashes the *docstring* into the metadata
/// buffer it checksums (`uniffi_macros`, `fnsig.rs`: the metadata expr
/// ends `.concat_long_str(#docstring)`), so two definitions that differ
/// only in having a doc comment get two different checksums. Bindings
/// are generated from the host cdylib and the shipped library is built
/// for Android — different `cfg`, different branch, different checksum —
/// and every FFI call died at load with "UniFFI API checksum mismatch"
/// in front of the user.
///
/// One export, one docstring, one checksum on every target. Anything
/// `cfg`-dependent belongs in the body, where it cannot reach the
/// metadata. `verifyRouteFfiAbi` in `:core:routing-android` fails the
/// build if this rule is ever broken again.
#[uniffi::export]
pub fn init_logging() {
    #[cfg(all(target_os = "android", feature = "logcat"))]
    android_logger::init_once(
        android_logger::Config::default().with_max_level(log::LevelFilter::Info),
    );
}
