# Unified routing: one cost field, every metre (incl. trails)

2026-05-30. Root-cause analysis + design for killing the "combined route
blindly follows tracks" pathology. Supersedes the graph/mesh **race** with a
single cost field.

## Symptom

Combined (default `foot`) routes snap to the nearest track and follow it
end-to-end, even when the track detours absurdly and cutting across terrain is
obviously cheaper. Tuning cannot fix it — it is structural.

## Root cause (confirmed in code)

1. **Graph edges are terrain-blind at request time.** `pathfinder.rs::
   edge_mul_closure` (WalkSeconds branch) builds the per-edge `EdgeContext`
   with **synthetic origin coordinates** (`fx:0, fy:0, tx:length, ty:0`,
   lines ~1551-1558). Every contributor that samples the world by coordinate
   (water, landcover, live slope) reads garbage at the EPSG:25833 origin. A
   trail edge's cost therefore comes only from **baked Naismith (length +
   baked slope) × flat trail discounts** (`PreferredEdge` 0.5× × `Marking`
   ~0.85×). A trail is structurally ~2-4× cheaper than identical open ground
   regardless of where it goes.

2. **Three solvers race on incomparable cost fields.** `solve_inner` computes
   on-graph, hybrid, and off-trail as independent whole-route candidates and
   takes the min by `cost`. The graph candidate is artificially cheap (1), so
   it wins on detours; and within the graph, Dijkstra **cannot leave the trail
   network mid-route** to shortcut. There is no single field where "follow this
   trail vs. cut across here" is decided per metre.

3. **The mesh's trail discount is ~5%, the graph's is 50-75%.** The single
   cost field where per-metre shortcutting IS possible (the mesh) barely
   prefers trails: `TrailProximityContributor` defaults to `bonus_at_zero ≈
   0.95` (`native_contributors.rs:1079-1082`) — a 5% break at the centreline.
   The *same physical thing* is costed an order of magnitude differently by the
   two solvers, so the mesh can never "win" a trail-following route and the
   system always falls back to the terrain-blind graph race.

## Data gaps found (same class as the regulated-lake gap)

`terrain.water_polygon` held only `lake` + `sea`. **Missing & now ingested:**
river-area polygons `elv` (22,247 / 1152 km²) → kind `river`; intermittent
freshwater `ferskvanntorrfall` (2,786 / 42 km²) → kind `river_dry`. Both are
real barriers that were crossed for free. (Other candidates noted for later:
`reingjerde` reindeer fences, `vegsperring` barriers, `naturvernomrade`
reserves — soft/None today, revisit if they cause issues.)

## Fix — one solver, one cost field

The mesh A* (grade-limited lifted solver) already costs **every cell at its
real coordinates** with the full contributor stack (slope, water integral,
total-gain, trail proximity). Make it **authoritative for all routing**, with
**trails represented as genuinely-cheap cells** (the real magnitude advantage a
trail has), so it follows a trail only while that is the cheaper per-metre
choice and departs the instant a shortcut is cheaper. The graph becomes a
**data source** (trail centrelines feeding the proximity field; exact geometry
for optional snap), not a competing router.

This is "apply the same cost model to every metre of track" — by construction,
because a trail is just a cheap cell in the same field as everything else.

### Increments

1. **Trail cells get real magnitude.** Strengthen `TrailProximityContributor`
   to a strong discount in a tight band around the centreline (≈ the graph's
   effective trail advantage), matched per fkb-type (sti/vei/skiløype). Keep
   the band ~1 cell wide so it's "on the trail," not a halo.
2. **Single solve is the default.** Route the combined default through the mesh
   solver start→end over this unified field. Stop racing the graph Dijkstra for
   the routing decision.
3. **Delete the race + legacy.** Remove `try_hybrid`, `CostMode::
   Multiplicative`, `off_trail_base`, the synthetic-coord graph closure path,
   and the dead Theta* body. Honest `NoRoute`/`NoCoverage` errors stay.
4. **Re-validate.** Trail-following hikes still hug trails; the blind-follow
   detour case cuts across; water gate stays clean; corpus holds; timing within
   budget.

## Step 1 — DONE & validated (2026-05-31)

- **Graph edge weight is now honest walk-seconds.** `Graph::route_with*` takes a
  `Fn(EdgeId, &EdgeRecord, baked) -> f32` closure returning the edge's ABSOLUTE
  cost (graph crate `lib.rs`); the Dijkstra no longer computes `baked × mul`.
  `pathfinder.rs::edge_cost_closure` composes the contributor stack and returns
  `total_walk_seconds`. Verified: a graph route now costs exactly `length ×
  BASE_PACE` (300 m → 214.29 s; 28 km → 20198 s) — same unit as the off-trail
  mesh `cost_seconds`, so the candidate-min race is finally apples-to-apples.
- **Trail discount right-sized.** `PreferredEdgeContributor` DNT/manual −50% →
  **−15%** (the −50% was calibrated for the old squared `baked×mul`; under
  honest walk-seconds it let marked trails win multi-× detours). The dominant
  on-trail advantage is now the off-trail roughness factor (~2.3×) trail edges
  don't pay.
- **Validated:** 12 combined-mode corpus hikes → all `on_graph`, **len/gt =
  1.00** (trail-following perfectly preserved), 0 water crossings, median
  829 ms. Water gate 59/60 (the 1 = finnmark honest no-route).
- **Tests restored (green):** the 3 strategy-selection tests
  (`pathfinder_picks_cheapest_strategy`, `cost_based_selection_beats_long_graph_
  detour`, `pathfinder_hybrid_when_one_end_off_graph`) failed only because
  off-trail is now FMM-only and needs a DEM to produce a candidate — their
  `None`-DEM fixtures meant OffTrail never entered the race. Added a `flat_dem_
  around()` fixture (single-tile constant-elevation DEM, `zstd` dev-dep) so the
  off-trail leg runs; relaxed the over-tight ±2 m length assert to grid-
  realistic slack (FMM walks a 10 m grid → ~148 m vs the ideal 141 m secant).
  All 6 pathfinder + graph + fmm crate tests pass.

## Step 2 — DONE & validated: `off_trail_factor` is now a contributor

Key finding: `off_trail_factor` is **multiplicative on the whole pace**
(`elastica.rs` `forward_cost`: `tobler × off × mul`), so it could NOT be a
drop-in *additive* contributor without changing slope behaviour. Resolved by
adding a **multiplicative `pace_factor` channel** to the cost model — the
architecturally correct fix, and the category future datasets (surface,
seasonal snow) need.

- `CostContributor::pace_factor(ctx) -> f64` (default 1.0). Composer applies
  `total = (base + Σ contribute) × Π pace_factor`.
- `OffTrailRoughnessContributor` (`pace_factor = n` on Mesh, 1.0 on Graph; no
  additive term). Added per-request to the off-trail solve stack via
  `fmm_adapter::with_off_trail` (it's per-request/profile, so not in the static
  Pathfinder stack; never touches graph edges).
- Solver no longer multiplies by `off`: dropped from `elastica.rs` `forward_cost`
  (both branches); the lazy overlay + `bake_contributor_pace` +
  `apply_contributor_factors_aniso` fold `Π pace_factor` into the per-cell `mul`.
  `GradeLimitedCost.off_trail_factor` survives only as the A* heuristic floor.
  (Anisotropic path keeps `off` in its metric and does NOT get the contributor —
  no double-count.)
- **Validated cost-neutral:** off-trail corpus 94.3 → 94.3 (+0.0), 56/59 flat
  (±<2 pt f32-ordering noise); water gate 59/60 (the 1 = finnmark no-route); all
  fmm + pathfind + graph crate tests pass.

## Step 3 — IN PROGRESS

**Done:** deleted `CostMode::Multiplicative` — the pre-Stage-2 `baked ×
legacy-multiplier` cost mode that was a second, incompatible cost definition
kept only as an A/B escape valve. Removed the enum variant + the
`edge_cost_closure` arm + the now-unused `compose_edge` import. Fully contained
(no other match sites); behaviorally inert (it was never the default — production
is `FastMarching`/walk-seconds). Combined route still `on_graph`, len/gt = 1.00;
all tests pass.

**Deferred (needs a design decision, not a rush):** the full single-solver merge
(one Dijkstra over graph + mesh) and deleting `try_hybrid` / the Theta* escape
valve. Rationale: post-Step-1 the 3-way race is an *honest* walk-seconds
comparison, so it is behaviorally correct (the blind-follow bug is gone) — this
is now architectural simplification, not a bugfix. And the obvious "rasterize
trails into a pure mesh solve" would **regress exact trail-following** (combined
routes currently hit len/gt = 1.00 precisely because the graph Dijkstra walks
exact trail polylines; a 10 m mesh only approximates them). The correct merge
keeps the trail graph for exact geometry and unifies it with the mesh into ONE
Dijkstra (graph edges + mesh-cell transitions in one structure, one walk-seconds
field) — a larger build that should preserve the len/gt = 1.00 property. Theta*
is still the documented escape valve for un-sizable FMM corridors (degenerate
from→to / no DEM), so its removal pairs with the merge.

### Unified-solver build — foundation landed + module spec

**Landed & compiling (reusable primitives):**
- `Graph::edge_ids_in_bbox(min,max,max_count) -> Vec<EdgeId>` — routing trail-
  splice primitive (`edges_in_bbox` only returns geometry).
- `turbo_tiles_fmm::tobler_pace` `pub` + re-exported — unified mesh edge cost
  uses the IDENTICAL slope pace as the lifted solver.
- `LazyContributorOverlay` + `::new` `pub(crate)` — reusable per-cell pace/veto
  for mesh edges.

**`crates/turbo-tiles-pathfind/src/unified.rs` (opt-in) — `solve_unified(...)`:**
one A* over a single `u32` node space:
- Mesh cells `[0, nx·ny)` (`compute_corridor_shape`); trail nodes `[nx·ny, …)` =
  distinct graph node ids among `edge_ids_in_bbox(corridor)`.
- **Mesh→mesh** (8-nbr): `step_m·tobler_pace(grad)·overlay.pace_mul(target)·steep`
  (mirrors lifted `forward_cost` minus heading; `refused`→skip; nodata→
  `step_m·BASE_PACE·3·mul`; `CLIFF_DEG=60`, `STEEP_PENALTY_K=10`).
- **Trail→trail**: `compose_edge_walk_seconds` on `EdgeKind::Graph(er)` per
  in-corridor `EdgeId` (Step-1 honest walk-seconds).
- **Transition** trail-node ↔ containing mesh cell: ~0 (join/leave anywhere).
- Heuristic `h = euclid·min_pace` (conservative/admissible).
- **Extract**: store `prev_edge` on trail→trail; splice exact
  `graph.edge_polyline(eid)` (oriented) → trail segments keep `len/gt=1.00`;
  mesh steps emit cell centres (Chaikin only the mesh portions). Build `Path`
  like the off-trail strategy (mirror `route_result_to_path` / off-trail build).
- **Opt-in**: `Prefs::unified: bool` (default false); `solve_inner` runs
  `solve_unified` instead of the race when set.
- **Gates before any default cut-over**: combined corpus len/gt≈1.00 on trail
  hikes; off-trail quality ≥ race; water 60/60; timing in budget.
- **v1 caveat**: 2D (no `(x,y,θ)` switchbacks). Heading-lifted unification (trail
  nodes as heading-free states in the lifted lattice) is v2 — cut to default only
  once it matches the lifted solver on steep off-trail.

### Known tradeoff

Very long (>~50 km) routes run the mesh over a large corridor (slower than the
graph's exact-polyline Dijkstra). Mitigated by the existing corridor cap +
adaptive cell. Acceptable for a hiking app; revisit if a real long-route case
regresses.
