//! Per-request layer trace + phase timing.
//!
//! When a caller asks for it (`Prefs::debug = true`), the pathfinder
//! installs a [`Tracer`] for the duration of the request and the
//! cost composers record per-layer call counts, timing, and cost
//! contributions into it. The snapshot lands in the response so
//! debugging stops being "comment out layers until it works".
//!
//! ## Design choices
//!
//! - **Thread-local guard.** A `TracerGuard` parks the tracer in a
//!   thread-local for the synchronous span of `solve()`. Composers
//!   peek at it via [`with`] without any extra arguments. Zero
//!   signature churn across the existing trait/compose surface.
//!   `solve()` doesn't await internally, so the thread-local stays
//!   pinned to the worker for the request's lifetime.
//!
//! - **Opt-in.** When no tracer is installed, [`with`] short-circuits
//!   on a single `RefCell::borrow` (a few ns). The hot path of the
//!   off-trail solver — hundreds of thousands of layer calls — pays
//!   nothing for instrumentation when debug is off.
//!
//! - **Single-threaded.** A request's cost composers run on one
//!   tokio worker (no scoped parallelism). The tracer's internal
//!   state uses `RefCell` instead of `Mutex`, saving ~15 ns per
//!   record vs. a mutex even under no contention.

use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde::Serialize;

#[derive(Debug, Default, Clone, Serialize)]
pub struct LayerStats {
    /// How many times the layer's `cell_cost` / `edge_cost_modifier`
    /// / `edge_multiplier` was called this request.
    pub calls: u64,
    /// Total wall-clock spent inside this layer's methods.
    pub elapsed_ns: u64,
    /// Sum of per-call cost contribution. Semantics depend on call
    /// site: for `cell_cost` it's the `multiplier - 1.0` sum, for
    /// edge layers it's accumulated "extra effective metres". Both
    /// answer "how much did this layer add to the final cost?"
    pub contribution_sum: f64,
    /// Cells / edges this layer pushed to refused (INFINITY).
    pub refusals: u64,
}

#[derive(Debug, Default, Clone, Serialize)]
pub struct PhaseTime {
    pub elapsed_ns: u64,
    pub calls: u32,
}

#[derive(Debug, Default, Clone, Serialize)]
pub struct MeshStats {
    pub cells: u32,
    pub refused_cells: u32,
    pub edges_relaxed: u64,
}

#[derive(Debug, Default, Clone, Serialize)]
pub struct TraceSnapshot {
    pub per_layer: Vec<NamedLayerStats>,
    pub phases: Vec<NamedPhase>,
    pub mesh: MeshStats,
}

#[derive(Debug, Default, Clone, Serialize)]
pub struct NamedLayerStats {
    pub name: String,
    #[serde(flatten)]
    pub stats: LayerStats,
}

#[derive(Debug, Default, Clone, Serialize)]
pub struct NamedPhase {
    pub name: String,
    #[serde(flatten)]
    pub phase: PhaseTime,
}

#[derive(Debug, Default)]
struct TracerInner {
    per_layer: HashMap<&'static str, LayerStats>,
    phases: HashMap<&'static str, PhaseTime>,
    mesh: MeshStats,
}

pub struct Tracer {
    inner: RefCell<TracerInner>,
}

impl Default for Tracer {
    fn default() -> Self {
        Self::new()
    }
}

impl Tracer {
    pub fn new() -> Self {
        Self {
            inner: RefCell::new(TracerInner::default()),
        }
    }

    /// Record one call to a layer's cost method.
    pub fn record_layer(
        &self,
        name: &'static str,
        elapsed: Duration,
        contribution: f64,
        refused: bool,
    ) {
        let mut g = self.inner.borrow_mut();
        let s = g.per_layer.entry(name).or_default();
        s.calls += 1;
        s.elapsed_ns = s.elapsed_ns.saturating_add(elapsed.as_nanos() as u64);
        if contribution.is_finite() {
            s.contribution_sum += contribution;
        }
        if refused {
            s.refusals += 1;
        }
    }

    /// Record one phase duration. Calling the same phase twice
    /// accumulates (e.g. "mesh_build" once for prefix and once for
    /// suffix when hybrid stitches three legs).
    pub fn record_phase(&self, name: &'static str, elapsed: Duration) {
        let mut g = self.inner.borrow_mut();
        let p = g.phases.entry(name).or_default();
        p.elapsed_ns = p.elapsed_ns.saturating_add(elapsed.as_nanos() as u64);
        p.calls += 1;
    }

    pub fn set_mesh_stats(&self, cells: u32, refused_cells: u32) {
        let mut g = self.inner.borrow_mut();
        g.mesh.cells = cells;
        g.mesh.refused_cells = refused_cells;
    }

    pub fn add_edges_relaxed(&self, n: u64) {
        let mut g = self.inner.borrow_mut();
        g.mesh.edges_relaxed = g.mesh.edges_relaxed.saturating_add(n);
    }

    pub fn snapshot(&self) -> TraceSnapshot {
        let g = self.inner.borrow();
        let mut per_layer: Vec<NamedLayerStats> = g
            .per_layer
            .iter()
            .map(|(k, v)| NamedLayerStats {
                name: (*k).to_string(),
                stats: v.clone(),
            })
            .collect();
        per_layer.sort_by_key(|n| n.name.clone());
        let mut phases: Vec<NamedPhase> = g
            .phases
            .iter()
            .map(|(k, v)| NamedPhase {
                name: (*k).to_string(),
                phase: v.clone(),
            })
            .collect();
        phases.sort_by_key(|n| n.name.clone());
        TraceSnapshot {
            per_layer,
            phases,
            mesh: g.mesh.clone(),
        }
    }
}

thread_local! {
    static ACTIVE: RefCell<Option<Arc<Tracer>>> = const { RefCell::new(None) };
}

/// Run `f` with the given tracer installed in this thread's slot.
/// The previous value (if any) is restored on drop, so nested
/// installs are safe (e.g. an inner debug request hijacked from a
/// hypothetical pipeline test).
pub fn with_installed<R>(tracer: Arc<Tracer>, f: impl FnOnce() -> R) -> R {
    let prev = ACTIVE.with(|cell| cell.replace(Some(tracer)));
    struct Restore(Option<Arc<Tracer>>);
    impl Drop for Restore {
        fn drop(&mut self) {
            let prev = self.0.take();
            ACTIVE.with(|cell| *cell.borrow_mut() = prev);
        }
    }
    let _r = Restore(prev);
    f()
}

/// Call `f` with a reference to the active tracer, if any. Returns
/// the closure's result. When no tracer is installed, `f` receives
/// `None` and the cost is one `RefCell::borrow` (~3 ns).
pub fn with<R>(f: impl FnOnce(Option<&Tracer>) -> R) -> R {
    ACTIVE.with(|cell| {
        let opt = cell.borrow();
        f(opt.as_deref())
    })
}

/// Convenience: scope a phase timing. Returns whatever the closure
/// returns. Equivalent to the explicit `Instant::now() + elapsed`
/// pattern but cheaper to write.
pub fn phase<R>(name: &'static str, f: impl FnOnce() -> R) -> R {
    let start = Instant::now();
    let r = f();
    let elapsed = start.elapsed();
    with(|t| {
        if let Some(t) = t {
            t.record_phase(name, elapsed);
        }
    });
    r
}

/// Convenience: record one layer call with timing. The cost
/// composer wraps each `layer.cell_cost()` etc. in this so the
/// stats are picked up automatically.
pub fn layer_call<R>(
    name: &'static str,
    f: impl FnOnce() -> R,
    contribution_for: impl FnOnce(&R) -> (f64, bool),
) -> R {
    let active = ACTIVE.with(|cell| cell.borrow().is_some());
    if !active {
        return f();
    }
    let start = Instant::now();
    let r = f();
    let elapsed = start.elapsed();
    let (contribution, refused) = contribution_for(&r);
    with(|t| {
        if let Some(t) = t {
            t.record_layer(name, elapsed, contribution, refused);
        }
    });
    r
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn installed_tracer_records_calls() {
        let t = Arc::new(Tracer::new());
        with_installed(t.clone(), || {
            let _ = layer_call("water", || 2.0_f32, |&m| ((m - 1.0) as f64, !m.is_finite()));
        });
        let snap = t.snapshot();
        let water = snap.per_layer.iter().find(|n| n.name == "water").unwrap();
        assert_eq!(water.stats.calls, 1);
        assert!((water.stats.contribution_sum - 1.0).abs() < 1e-3);
    }

    #[test]
    fn no_tracer_is_a_noop_short_circuit() {
        // Without an installed tracer, layer_call still runs `f`
        // and returns its value but pays no extra time. Smoke test
        // for "didn't accidentally do work in disabled mode".
        let r = layer_call(
            "water",
            || 1.5_f32,
            |_| panic!("contribution_for must NOT be called when disabled"),
        );
        assert!((r - 1.5).abs() < 1e-3);
    }

    #[test]
    fn phase_timing_accumulates() {
        let t = Arc::new(Tracer::new());
        with_installed(t.clone(), || {
            phase("snap", || std::thread::sleep(Duration::from_millis(1)));
            phase("snap", || std::thread::sleep(Duration::from_millis(1)));
        });
        let snap = t.snapshot();
        let p = snap.phases.iter().find(|p| p.name == "snap").unwrap();
        assert_eq!(p.phase.calls, 2);
        assert!(p.phase.elapsed_ns > 1_000_000); // >1ms
    }
}
