//! Per-event solver recording for the algorithm visualizer.
//!
//! When `Prefs::record` is set, the pathfinder installs a
//! [`Recorder`] on a thread-local slot for the duration of the
//! solve. Theta* and Dijkstra push one [`SolverEvent`] per pop,
//! per edge relaxation, per line-of-sight cast, and one
//! `BestPath` event when the algorithm finishes reconstruction.
//! The admin SPA replays the resulting [`SolverRecording`] as an
//! animated "snake trail" — exactly the visualization the curator
//! has been asking for to make calibration decisions inspectable.
//!
//! ## Design choices
//!
//! - **Thread-local install pattern**, identical to [`crate::tracer`].
//!   No signature churn across `theta_star`, `Dijkstra`, or any
//!   other algorithm we later add. Solvers call [`record`] which
//!   short-circuits on a single `RefCell::borrow` when no recorder
//!   is installed (~3 ns).
//!
//! - **Capacity-bounded with decimation**. A 5 km Marka query
//!   produces ~40 K mesh edges and several million Dijkstra
//!   relaxations. Recording every one would OOM the response and
//!   crush the SPA animation. The recorder accepts an
//!   `event_cap`; when the cap is hit, every further event is
//!   subject to deterministic decimation (keep 1 in N where N
//!   grows over time). The animation still shows the
//!   characteristic exploration pattern, just with fewer frames.
//!
//! - **Coordinates in EPSG:25833 metres at record time**. The
//!   `Pathfinder` converts to WGS84 once when serialising the
//!   recording, not per event. Keeps the recorder allocation-free
//!   on the hot path.
//!
//! - **Phase frames**. Events are grouped into phases that match
//!   the existing `tracer::phase` boundaries (`try_on_graph`,
//!   `try_hybrid`, `mesh_inputs`, `build_local_mesh`, `theta_star`)
//!   so the SPA can show "events during theta_star" in isolation.
//!
//! ## What the SPA does with this
//!
//! Each event renders as a small MapLibre primitive:
//!
//! | Event              | Rendered as                           |
//! |--------------------|---------------------------------------|
//! | NodePopped         | Faint blue dot, "explored" layer       |
//! | EdgeRelaxed        | Orange line, fades over N frames       |
//! | LineOfSightCast    | Dashed yellow (hit) / red (blocked)    |
//! | BestPathSnapshot   | Blue polyline that replaces itself     |
//! | MeshBuilt          | One-shot "this many cells" overlay     |
//!
//! Together they form a "snake trail" of exploration that the
//! curator can play, pause, scrub, and step through to understand
//! what cost model assumptions the solver was actually optimising.

use std::cell::RefCell;
use std::sync::Arc;

use serde::Serialize;

/// One recorded solver event. Coordinates are EPSG:25833 metres at
/// record time; the pathfinder converts to WGS84 on serialise
/// (record+replay path) or on each SSE-event emit (live path).
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SolverEvent {
    /// Mesh-build completed for this query. One-shot — emitted
    /// before any node pops.
    MeshBuilt { cells: u32, refused_cells: u32 },
    /// A node was popped from the priority queue. `g` is the
    /// known cost from start to this node; `h` is the heuristic
    /// to goal. Together they characterise the frontier shape.
    NodePopped { x: f32, y: f32, g: f32, h: f32 },
    /// Edge relaxation. `took_los` is true when the relaxation
    /// used a Theta* line-of-sight shortcut rather than the mesh
    /// edge between `from` and `to`. The SPA renders LoS shortcut
    /// relaxations differently so the curator can see exactly
    /// where the any-angle optimisation fired.
    EdgeRelaxed {
        fx: f32,
        fy: f32,
        tx: f32,
        ty: f32,
        new_g: f32,
        took_los: bool,
    },
    /// Line-of-sight cast. Emitted for every test, hit or miss.
    LineOfSightCast {
        fx: f32,
        fy: f32,
        tx: f32,
        ty: f32,
        blocked: bool,
    },
    /// Current best-path snapshot. Emitted only at the end of
    /// reconstruction so the SPA can show the answer snapping
    /// into place. Coordinates are EPSG:25833 metres.
    BestPathSnapshot { coords: Vec<[f32; 2]> },
}

/// One phase frame holding the events emitted during a single
/// `tracer::phase` boundary.
#[derive(Debug, Clone, Serialize)]
pub struct PhaseFrame {
    pub name: String,
    pub started_at_us: u64,
    pub events: Vec<SolverEvent>,
}

/// Top-level serialised recording — what the API returns and the
/// SPA replays.
#[derive(Debug, Clone, Serialize)]
pub struct SolverRecording {
    pub phases: Vec<PhaseFrame>,
    /// True when decimation kicked in. The SPA renders a "(showing
    /// 1 in N events)" notice so the curator knows the animation
    /// is approximate, not lossless.
    pub decimated: bool,
    /// Total events that would have been emitted without
    /// decimation. `phases[*].events` sums to a smaller number
    /// when decimated.
    pub events_observed: u64,
    /// Total events actually retained.
    pub events_retained: u64,
}

/// The recorder. One per `Pathfinder::solve` call, installed on a
/// thread-local. All solver callbacks go through [`record`].
///
/// In addition to building an in-memory [`SolverRecording`], a
/// recorder can fan out each event to a tokio mpsc channel for
/// live SSE streaming. The streaming side uses `try_send` so a
/// slow consumer drops frames instead of backpressuring the hot
/// solver loop — the SPA's live overlay tolerates frame drops the
/// same way the in-memory recorder tolerates decimation.
pub struct Recorder {
    cap: u64,
    inner: RefCell<RecorderInner>,
    /// Optional fan-out for live streaming. Empty for the
    /// record+replay path; populated by `new_streaming` for the
    /// SSE endpoint. `Sender` is `Clone` + `Send`; `try_send`
    /// avoids blocking the solver when the channel is full.
    tx: Option<tokio::sync::mpsc::Sender<SolverEvent>>,
    /// Per-recorder dropped-event counter, surfaced on the
    /// snapshot so the SPA / scripts can flag "stream dropped N
    /// events" rather than silently truncating.
    dropped_to_channel: std::cell::Cell<u64>,
}

#[derive(Debug)]
struct RecorderInner {
    /// All phases recorded so far, oldest first.
    phases: Vec<PhaseFrame>,
    /// Number of events observed (pre-decimation).
    observed: u64,
    /// Number of events kept (post-decimation).
    retained: u64,
    /// Started instant for relative timestamps in phase frames.
    started_at: std::time::Instant,
    /// When non-1, every Nth event is kept. Updated on the fly
    /// once `retained` approaches `cap` so the recording stays
    /// bounded even for million-event solves.
    keep_every: u64,
}

impl Recorder {
    /// Create a new in-memory-only recorder with the given event
    /// cap. Once the cap is approached, decimation kicks in to
    /// keep the recording bounded. A typical interactive request
    /// uses `cap=200_000`.
    pub fn new(cap: u64) -> Self {
        Self::with_channel(cap, None)
    }

    /// Create a recorder that ALSO fans out every event to the
    /// given tokio mpsc Sender. Used by the SSE streaming endpoint
    /// so live consumers see the solver's exploration as it
    /// happens. Channel backpressure is handled via `try_send` —
    /// dropped events are counted but the solver continues at
    /// full speed.
    pub fn new_streaming(cap: u64, tx: tokio::sync::mpsc::Sender<SolverEvent>) -> Self {
        Self::with_channel(cap, Some(tx))
    }

    fn with_channel(cap: u64, tx: Option<tokio::sync::mpsc::Sender<SolverEvent>>) -> Self {
        Self {
            cap: cap.max(1),
            inner: RefCell::new(RecorderInner {
                phases: Vec::new(),
                observed: 0,
                retained: 0,
                started_at: std::time::Instant::now(),
                keep_every: 1,
            }),
            tx,
            dropped_to_channel: std::cell::Cell::new(0),
        }
    }

    /// Start a new phase frame. Subsequent `record` calls land in
    /// this phase until another `begin_phase` arrives. Idempotent
    /// — calling with the same name re-uses the current phase if
    /// it's already open (matches `tracer::phase`'s nested-but-
    /// sequential semantics).
    pub fn begin_phase(&self, name: &str) {
        let mut g = self.inner.borrow_mut();
        if g.phases.last().map(|p| p.name == name).unwrap_or(false) {
            return;
        }
        let started_at_us = g.started_at.elapsed().as_micros() as u64;
        g.phases.push(PhaseFrame {
            name: name.to_string(),
            started_at_us,
            events: Vec::new(),
        });
    }

    /// Record one event. May be decimated when the recording is
    /// near its event cap. Always increments `observed`.
    pub fn record(&self, event: SolverEvent) {
        let cap = self.cap;
        let mut g = self.inner.borrow_mut();
        g.observed = g.observed.saturating_add(1);
        // Adaptive decimation: double `keep_every` whenever retained
        // hits 80% of cap. This keeps the recording asymptotically
        // bounded even for million-event solves while preserving
        // early frames at full fidelity.
        if g.retained >= (cap.saturating_mul(8) / 10) {
            g.keep_every = g.keep_every.saturating_mul(2).max(2);
        }
        let keep_in_memory = g.observed.is_multiple_of(g.keep_every);
        if keep_in_memory {
            if g.phases.is_empty() {
                let started_at_us = g.started_at.elapsed().as_micros() as u64;
                g.phases.push(PhaseFrame {
                    name: "unknown".to_string(),
                    started_at_us,
                    events: Vec::new(),
                });
            }
            g.phases.last_mut().unwrap().events.push(event.clone());
            g.retained = g.retained.saturating_add(1);
        }
        // Drop the inner borrow before doing channel work — the
        // tx send is non-blocking but might allocate.
        drop(g);
        if let Some(tx) = self.tx.as_ref() {
            if tx.try_send(event).is_err() {
                // Channel full or closed — best-effort streaming;
                // bump the counter so the snapshot reports it.
                self.dropped_to_channel
                    .set(self.dropped_to_channel.get().saturating_add(1));
            }
        }
    }

    pub fn snapshot(&self) -> SolverRecording {
        let g = self.inner.borrow();
        SolverRecording {
            phases: g.phases.clone(),
            decimated: g.keep_every > 1,
            events_observed: g.observed,
            events_retained: g.retained,
        }
    }
}

thread_local! {
    static ACTIVE: RefCell<Option<Arc<Recorder>>> = const { RefCell::new(None) };
    // UTM coords (EPSG:25833) prepended to every `BestPathSnapshot`.
    // Used by multi-waypoint `solve_route` so the live preview shows the
    // already-finalized earlier legs while a later leg is still solving,
    // instead of the preview jumping back to the current leg's start.
    static SNAPSHOT_PREFIX: RefCell<Vec<[f32; 2]>> = const { RefCell::new(Vec::new()) };
}

/// Set the coords (UTM33N) prepended to subsequent `BestPathSnapshot`
/// events. Pass an empty vec to clear. Cheap; intended to be set at
/// per-leg boundaries by `Pathfinder::solve_route`.
pub fn set_snapshot_prefix(coords: Vec<[f32; 2]>) {
    SNAPSHOT_PREFIX.with(|cell| *cell.borrow_mut() = coords);
}

/// Run `f` with the given recorder installed for the duration. The
/// previous value (if any) is restored on drop, so nested installs
/// are safe.
pub fn with_installed<R>(rec: Arc<Recorder>, f: impl FnOnce() -> R) -> R {
    let prev = ACTIVE.with(|cell| cell.replace(Some(rec)));
    struct Restore(Option<Arc<Recorder>>);
    impl Drop for Restore {
        fn drop(&mut self) {
            let prev = self.0.take();
            ACTIVE.with(|cell| *cell.borrow_mut() = prev);
        }
    }
    let _r = Restore(prev);
    f()
}

/// Push one event onto the active recorder, if any. The ~3 ns
/// no-recorder cost (one `RefCell::borrow`) keeps the hot Theta*
/// loop cheap when recording is disabled.
pub fn record(event_fn: impl FnOnce() -> SolverEvent) {
    let has = ACTIVE.with(|cell| cell.borrow().is_some());
    if !has {
        return;
    }
    let mut event = event_fn();
    // Prepend the per-leg prefix so a multi-waypoint live preview shows
    // the whole route built so far, not just the current leg.
    if let SolverEvent::BestPathSnapshot { coords } = &mut event {
        SNAPSHOT_PREFIX.with(|cell| {
            let prefix = cell.borrow();
            if !prefix.is_empty() {
                let mut full = Vec::with_capacity(prefix.len() + coords.len());
                full.extend_from_slice(&prefix);
                full.append(coords);
                *coords = full;
            }
        });
    }
    ACTIVE.with(|cell| {
        if let Some(r) = cell.borrow().as_ref() {
            r.record(event);
        }
    });
}

/// Mark the start of a new phase on the active recorder, if any.
pub fn begin_phase(name: &str) {
    ACTIVE.with(|cell| {
        if let Some(r) = cell.borrow().as_ref() {
            r.begin_phase(name);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_recorder_is_a_noop() {
        // record() with no install should be safe and cheap; the
        // closure that builds the event must NOT run, since
        // constructing a SolverEvent allocates for BestPathSnapshot.
        record(|| panic!("event closure ran with no recorder installed"));
    }

    #[test]
    fn installed_recorder_captures_events() {
        let rec = Arc::new(Recorder::new(1000));
        with_installed(rec.clone(), || {
            begin_phase("theta_star");
            record(|| SolverEvent::NodePopped {
                x: 1.0,
                y: 2.0,
                g: 0.0,
                h: 5.0,
            });
            record(|| SolverEvent::NodePopped {
                x: 3.0,
                y: 4.0,
                g: 1.0,
                h: 4.0,
            });
        });
        let snap = rec.snapshot();
        assert_eq!(snap.phases.len(), 1);
        assert_eq!(snap.phases[0].name, "theta_star");
        assert_eq!(snap.phases[0].events.len(), 2);
        assert_eq!(snap.events_observed, 2);
        assert_eq!(snap.events_retained, 2);
        assert!(!snap.decimated);
    }

    #[test]
    fn decimation_kicks_in_above_cap() {
        // Cap=100 → decimation starts around 80 events. Push 5_000
        // events and confirm we don't keep more than ~cap.
        let rec = Arc::new(Recorder::new(100));
        with_installed(rec.clone(), || {
            begin_phase("p");
            for i in 0..5000 {
                record(|| SolverEvent::NodePopped {
                    x: i as f32,
                    y: 0.0,
                    g: i as f32,
                    h: 0.0,
                });
            }
        });
        let snap = rec.snapshot();
        assert_eq!(snap.events_observed, 5000);
        assert!(snap.decimated, "should have decimated");
        // Retained should stay within an order of magnitude of cap.
        assert!(
            snap.events_retained <= 200,
            "retained={}",
            snap.events_retained
        );
        assert!(
            snap.events_retained >= 50,
            "retained={}",
            snap.events_retained
        );
    }

    #[test]
    fn phase_boundaries_track() {
        let rec = Arc::new(Recorder::new(1000));
        with_installed(rec.clone(), || {
            begin_phase("a");
            record(|| SolverEvent::NodePopped {
                x: 0.0,
                y: 0.0,
                g: 0.0,
                h: 0.0,
            });
            begin_phase("b");
            record(|| SolverEvent::NodePopped {
                x: 1.0,
                y: 1.0,
                g: 1.0,
                h: 0.0,
            });
        });
        let snap = rec.snapshot();
        assert_eq!(snap.phases.len(), 2);
        assert_eq!(snap.phases[0].name, "a");
        assert_eq!(snap.phases[1].name, "b");
        assert_eq!(snap.phases[0].events.len(), 1);
        assert_eq!(snap.phases[1].events.len(), 1);
    }
}
