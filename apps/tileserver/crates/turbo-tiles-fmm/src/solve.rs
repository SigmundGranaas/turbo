//! Fast Marching marching loop, 2D isotropic.
//!
//! Phase 1 implementation: a concrete solver that takes a per-cell
//! cost field `f = 1/F` (seconds per metre) and produces an
//! arrival-time field `u(x)`. Phase 2 will generalise this to a
//! `Metric` trait so the same loop drives anisotropic (Tobler-
//! Finsler) and curvature-augmented (Euler-elastica) metrics.
//!
//! The marching loop is the textbook Sethian narrow-band scheme:
//!
//! ```text
//!   initialise u[seed] = 0, push seed into heap
//!   while heap not empty:
//!     (u_a, a) = pop_min()
//!     if a is ACCEPTED: continue       # stale entry from decrease_key
//!     mark a ACCEPTED
//!     u[a] = u_a
//!     for each 4-neighbour b of a:
//!       if b is ACCEPTED: continue
//!       u_b_new = solve_quadratic_2d(u_x, u_y, f_inv, h)
//!         where u_x = min over the x-axis neighbours of b that
//!         are ACCEPTED, u_y same on y axis.
//!       if u_b_new < u[b]: decrease_key(b, u_b_new)
//! ```
//!
//! Causality: a node is only ACCEPTED when popped, and every node
//! popped has the lowest CONSIDERED key, so by induction it's also
//! the lowest possible arrival time. The Sethian quadratic update
//! is monotone in its accepted-neighbour inputs, so re-relaxation
//! never produces a *higher* value once a cell's neighbours stop
//! changing.

use crate::grid::{FmmGrid, GridShape};
use crate::heap::NarrowBandHeap;
use crate::metric::{LocalCost, Metric};
use crate::stencil::solve_quadratic_2d;

/// One of three states for every cell during the marching loop.
/// Packed into a `u8` for memory locality — the per-cell state
/// vector is the second-largest allocation after the arrival-time
/// grid.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
#[derive(Default)]
pub enum NodeState {
    /// Not yet reached by the wave. `u = +∞`.
    #[default]
    Far = 0,
    /// In the heap; has at least one ACCEPTED neighbour so a
    /// candidate `u` has been computed.
    Considered = 1,
    /// Final value committed; never updated again.
    Accepted = 2,
}

/// Result of one FMM solve. Holds the arrival-time field plus
/// statistics for diagnostics. `came_from` will be added in
/// phase 4 when path extraction needs it.
#[derive(Debug)]
pub struct FmmResult {
    pub arrival: FmmGrid<f32>,
    pub cells_accepted: u32,
}

/// Optional termination condition for `solve_2d_isotropic`. The
/// path extractor only needs the field up to the goal cell; for
/// debug + diagnostic visualisations we want the whole field.
#[derive(Debug, Clone, Copy)]
pub enum StopCondition {
    /// March until the heap is empty. Produces the full field.
    AllAccepted,
    /// March until the goal cell has been ACCEPTED. The arrival
    /// values past the goal are left at `+∞` / partially computed
    /// — fine for path extraction (which walks from goal back).
    GoalReached { gi: u32, gj: u32 },
}

/// Bake a `Metric` into a flat per-cell cost grid. Called once at
/// the start of a solve so the hot relaxation loop reads from a
/// dense `Vec<f32>` instead of going through dyn dispatch on every
/// neighbour visit. Refused cells get `f32::INFINITY`.
pub fn bake_metric_2d<M: Metric>(metric: &M, shape: GridShape) -> FmmGrid<f32> {
    debug_assert_eq!(shape.nz, 1, "2D bake only");
    let mut cost = FmmGrid::filled(shape, 1.0_f32);
    for j in 0..shape.ny {
        for i in 0..shape.nx {
            let c = match metric.local(&shape, i, j, 0) {
                LocalCost::Walkable { pace_s_per_m } => pace_s_per_m,
                LocalCost::Refused => f32::INFINITY,
            };
            cost.set(i, j, 0, c);
        }
    }
    cost
}

/// Solve the eikonal equation on a 2D regular grid using a `Metric`
/// to source per-cell cost. Phase 2 entry point.
pub fn solve_2d_with_metric<M: Metric>(
    shape: GridShape,
    metric: &M,
    seeds: &[(u32, u32, f32)],
    stop: StopCondition,
) -> FmmResult {
    let cost = bake_metric_2d(metric, shape);
    solve_2d_isotropic(shape, &cost, seeds, stop)
}

/// Solve the eikonal equation on a 2D regular grid.
///
/// ## Arguments
///
/// * `cost` — per-cell `f = 1/F` in seconds-per-metre. Cells with
///   `cost = +∞` are refused: they're never accepted and contribute
///   no upwind value to their neighbours. Shape must match `shape`.
/// * `seeds` — `(i, j, u0)` triples giving the initial arrival
///   times. Usually one seed at `u0 = 0`, but the elastica solver
///   in phase 5 will seed all `n_theta` headings at once.
/// * `stop` — when to terminate.
///
/// ## Returns
///
/// `FmmResult` with the arrival-time field. Refused / unreached
/// cells stay at `+∞`.
pub fn solve_2d_isotropic(
    shape: GridShape,
    cost: &FmmGrid<f32>,
    seeds: &[(u32, u32, f32)],
    stop: StopCondition,
) -> FmmResult {
    debug_assert_eq!(shape.nz, 1, "phase 1 solver is 2D; use phase 5 for nz > 1");
    debug_assert_eq!(cost.shape.len(), shape.len());

    let n = shape.len();
    let mut arrival: FmmGrid<f32> = FmmGrid::filled(shape, f32::INFINITY);
    let mut state: Vec<NodeState> = vec![NodeState::Far; n];
    let mut heap = NarrowBandHeap::with_cells(n);
    let h = shape.cell_m as f32;
    let nx = shape.nx;
    let ny = shape.ny;

    // Seed initialisation. Each seed is pushed at its given key;
    // it'll be accepted on the next pop_min iteration.
    for &(si, sj, u0) in seeds {
        if si >= nx || sj >= ny {
            continue;
        }
        let flat = shape.idx(si, sj, 0);
        // Skip refused seeds — they'd never be accepted anyway.
        if !cost.flat()[flat].is_finite() {
            continue;
        }
        if u0 < arrival.flat()[flat] {
            arrival.flat_mut()[flat] = u0;
            state[flat] = NodeState::Considered;
            heap.push(u0, flat as u32);
        }
    }

    let mut cells_accepted: u32 = 0;

    while let Some((u_a, cell_a)) = heap.pop_min() {
        let flat_a = cell_a as usize;
        // Stale-key skip: the popped key may be larger than the
        // current arrival[a] if the cell was decreased after being
        // pushed under an older key. We compare against the field
        // (always authoritative) rather than the heap key.
        if state[flat_a] == NodeState::Accepted {
            continue;
        }
        if arrival.flat()[flat_a] < u_a {
            // Should not happen with `decrease_key_or_insert`, but
            // skip defensively to preserve monotonicity.
            continue;
        }
        state[flat_a] = NodeState::Accepted;
        cells_accepted += 1;

        let (i, j, _) = shape.unpack(flat_a);

        if let StopCondition::GoalReached { gi, gj } = stop {
            if i == gi && j == gj {
                return FmmResult {
                    arrival,
                    cells_accepted,
                };
            }
        }

        // Relax the four axis-aligned neighbours.
        // The Sethian stencil uses ONLY the four cardinal directions
        // in 2D. Diagonals are reached through the quadratic update
        // combining x and y axes; adding diagonal neighbours would
        // overcount paths.
        const STEPS: [(i32, i32); 4] = [(-1, 0), (1, 0), (0, -1), (0, 1)];
        for (di, dj) in STEPS.iter().copied() {
            let ni = i as i32 + di;
            let nj = j as i32 + dj;
            if ni < 0 || nj < 0 || ni as u32 >= nx || nj as u32 >= ny {
                continue;
            }
            let nflat = shape.idx(ni as u32, nj as u32, 0);
            if state[nflat] == NodeState::Accepted {
                continue;
            }
            let f_inv = cost.flat()[nflat];
            if !f_inv.is_finite() {
                // Refused neighbour cannot be relaxed. Its arrival
                // stays at +∞ and the wave routes around.
                continue;
            }

            // Gather the minimum ACCEPTED neighbour value on each
            // axis. This is the upwind constraint — only neighbours
            // already accepted (i.e. lower or equal arrival time)
            // contribute. Considered / Far neighbours contribute +∞.
            let u_x = axis_min_accepted(ni as u32, nj as u32, &arrival, &state, &shape, AxisDir::X);
            let u_y = axis_min_accepted(ni as u32, nj as u32, &arrival, &state, &shape, AxisDir::Y);

            let u_candidate = solve_quadratic_2d(u_x, u_y, f_inv, h);
            if u_candidate < arrival.flat()[nflat] {
                arrival.flat_mut()[nflat] = u_candidate;
                state[nflat] = NodeState::Considered;
                heap.decrease_key_or_insert(nflat as u32, u_candidate);
            }
        }
    }

    FmmResult {
        arrival,
        cells_accepted,
    }
}

/// Which axis to scan for the minimum ACCEPTED neighbour.
#[derive(Clone, Copy)]
enum AxisDir {
    X,
    Y,
}

/// Minimum arrival time among the two ACCEPTED neighbours along
/// `axis`. Returns `+∞` when neither neighbour is accepted (i.e.
/// this axis can't contribute to the Sethian quadratic at this
/// cell yet).
#[inline]
fn axis_min_accepted(
    i: u32,
    j: u32,
    arrival: &FmmGrid<f32>,
    state: &[NodeState],
    shape: &GridShape,
    axis: AxisDir,
) -> f32 {
    let (di, dj) = match axis {
        AxisDir::X => (1i32, 0i32),
        AxisDir::Y => (0i32, 1i32),
    };
    let mut best = f32::INFINITY;
    for sign in [-1i32, 1i32] {
        let ni = i as i32 + sign * di;
        let nj = j as i32 + sign * dj;
        if ni < 0 || nj < 0 || ni as u32 >= shape.nx || nj as u32 >= shape.ny {
            continue;
        }
        let nflat = shape.idx(ni as u32, nj as u32, 0);
        if state[nflat] == NodeState::Accepted {
            let u = arrival.flat()[nflat];
            if u < best {
                best = u;
            }
        }
    }
    best
}
