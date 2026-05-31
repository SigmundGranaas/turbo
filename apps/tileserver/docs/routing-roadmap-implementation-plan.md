# Routing roadmap — complete implementation plan

_2026-05-30. The end-state, built properly. No temporary mitigations, no
band-aids. Legacy/fallback paths that are strictly worse are deleted, not
kept "for A/B". Where a route genuinely can't be produced, the solver returns
a **specific, actionable error** — never a plausible-looking wrong route._

---

## Principles (non-negotiable)

1. **One solver, one cost model.** A single state-augmented A\* over a unified
   terrain+trail cost field. No parallel "strategies" that each get a route
   and a vote.
2. **No band-aids.** No `off_trail_base` flat multiplier standing in for real
   trail cost; no "interim" tuning. Costs are physical (pace s/m by surface +
   slope + gain), refusals are hard, preferences emerge from the cost field.
3. **Delete what's worse.** Theta\* mesh fallback, the hybrid bridge, the
   multiplicative `CostMode`, the legacy mesh builder — removed. They produce
   worse routes than the FMM/lifted solver and exist only as fallbacks.
4. **Honest failure.** Goal unreachable ⇒ structured error naming the cause
   (water / cliff / coverage gap / out-of-range) and where. Never a straight
   line across a fjord.
5. **Every change is gated.** `water_check.py` (zero hard crossings) + the
   terrain corpus + a timing budget + a visual check. The water-blindness
   regression taught us a composite score is not proof.

---

## Target architecture: the unified solve

```
solve(from, to, prefs):
  corridor  = adaptive_corridor(from, to)          # O(d) area, adaptive cell
  field     = CostField(corridor):                 # built lazily per visited cell
                terrain : Tobler(grade) + grade-cap refusal (switchbacks)
                surface : trail cells → trail-surface pace (sti 1.0×, vei 1.6×…)
                          off-trail cells → off-trail terrain pace
                refusal : deep water / glacier / true cliff → ∞
  path      = astar_lifted(field, from, to,        # (x,y,heading) lattice
                heuristic = remaining_straightline × min_pace,
                on_progress = stream best-path-so-far)
  if path is None: return UnreachableError(cause, location)
  route     = snap_on_trail_segments_to_polyline(path)   # exact trail geometry
  return route
```

### Why this fixes all three problems
- **Speed:** A\* (goal-directed, not flooding) + O(d) corridor + adaptive
  cell + lazy per-cell cost ⇒ minutes → seconds.
- **Detour-following:** trails are *cheap cells*, not a flat bonus. The
  optimal path rides a trail exactly while the trail is cheaper than cutting
  across, and leaves it the moment a cross-country step costs less —
  evaluated at every cell, not as a whole-route choice. The Gamli-fjellet
  detour disappears because the detour cells cost more than the shortcut.
- **Progress:** the A\* `on_progress` callback reconstructs the best path to
  the frontier each tick and streams it, so the UI shows a trail reaching
  toward the goal and refining.

### Trail cost replaces `off_trail_base`
Each corridor cell gets a **surface pace** baked from the rasterised trail
graph: a cell on a `sti` pays ~1.0× base pace, `vei` ~1.6×, etc.; an
off-trail cell pays the Tobler terrain pace for its slope. "Prefer trails"
is then a *consequence of physics*, not a 2.3× thumb on the scale. Leaving a
trail is worth it precisely when terrain pace < trail pace + detour length —
the correct condition.

### Scale: corridor vs long-haul
- The unified corridor solve covers routes up to `max_corridor_km`
  (config, default ~40 km) — the entire realistic hiking range.
- Beyond that, a raster corridor is infeasible (gigabytes). The trail-graph
  Dijkstra is the **scale-appropriate** algorithm there, sharing the *same*
  surface/Tobler/gain cost model — not a worse fallback, the right tool. If
  the endpoints have no trail-network connection at that range, return a
  clear out-of-range error rather than a mesh guess.

### Honest errors (replace every silent-fallback)
| Situation | Old behaviour | New behaviour |
|---|---|---|
| Goal walled by water/cliff | Theta\* line across it | `Unreachable{cause: Water, near: [lon,lat]}` |
| No DEM/mask coverage | straight-line "route" | `NoCoverage{which}` (already exists — keep) |
| Endpoint in lake/glacier | opaque NoRoute | `EndpointRefused{which, layer}` (keep) |
| Route > max_corridor_km, no trail path | — | `OutOfRange{dist_km, max_km}` |
| Corridor exhausted, goal isolated | fallback | `Unreachable{cause: Isolated}` + nearest reachable point |

---

## What gets DELETED

| Removed | Why | Replaced by |
|---|---|---|
| `core::off_trail` Theta\* (`theta_star`, `Mesh`, `mesh_inputs_for_bbox`) | blocky line-of-sight routes; the fallback the user kept seeing | unified A\* |
| FMM→Theta\* fallback in `build_off_trail_segment` | masks real failures with garbage | honest `Unreachable` error |
| `try_hybrid` + `stitch_hybrid` | off-trail only at endpoints; can't cut mid-route detours | unified solve (off-trail everywhere) |
| 3-way strategy dispatch in `solve_inner` | whole-route voting; root of detour-following | single solve |
| `CostMode::Multiplicative` (+ `compose_*` multiplicative path) | legacy "A/B escape valve" | walk-seconds cost field only |
| `off_trail_base` knob | band-aid for missing trail cost | per-cell surface pace |
| 2D FMM path (`solve_fmm_corridor`, `solve_2d_anisotropic`, `extract_path*`) | redundant once lifted A\* is the sole solver | unified lifted A\* (**only after it's proven ≥ as fast/good on gentle terrain — that's the bar, not a reason to keep two**) |

---

## Phased implementation

Each phase is independently shippable and ends at a green gate. Phases 1–2
make the *current* solver fast and observable; Phase 3 unifies and deletes.

### Phase 0 — Verification foundation (do first)
- Promote `tools/water_check.py` to the mandatory gate: 0 hard crossings or
  the change is rejected.
- Add a **cost-effectiveness / detour metric** to `terrain_metrics.py`:
  route cost vs the lower of (straight-line terrain cost, trail-follow cost),
  so detour-following is *measurable* before and after Phase 3.
- Add a **timing budget** assertion to the corpus (per-route wall-clock cap).
- **Gate:** harness runs, baselines recorded.

### Phase 1 — Make the solve fast (A\*, corridor, adaptive cell)
Files: `elastica.rs`, `fmm_adapter.rs`, `config.rs`, `heap.rs`.
- **A\*:** store `g` separately; push `g + h` to `NarrowBandHeap`, where
  `h = euclidean_to_goal × min_pace_s_per_m` (admissible). Goal test on pop.
- **Corridor cap:** `pad = min(0.30·d, 1.2 km)`, `half_width = min(0.20·d,
  1.2 km)` → O(d) area. Config-exposed.
- **Adaptive cell:** `cell_m = 10` if `d ≤ 3 km` else `20`. Config-exposed.
- **Lazy cost:** compute the per-cell refusal/surface overlay on first visit
  (A\* touches a small fraction), not eagerly over the whole corridor;
  water refusal from the mask raster (O(1)) instead of the spoke-test.
- **Progress hook:** add `on_progress: Option<&mut dyn FnMut(Progress)>` to
  the solver signature (wired in Phase 2; no-op now).
- **Gate:** the 11.5 km route < ~5 s; `water_check` 0 hard crossings; corpus
  composite ≥ current 94.7; switchback + lake unit tests pass.

### Phase 2 — Live progress (watch trails build)
Files: `elastica.rs` (emit), `fmm_adapter.rs`, `pathfind.rs` (stream),
`PlotRoute.tsx` (render).
- Solver emits, every ~50 k pops: explored count, frontier size,
  closest-to-goal distance, and a **best-path-so-far** polyline
  (backtrack `parent` from the min-`g+h` frontier state).
- Stream over the existing SSE `/v1/pathfind/stream` — but make it run the
  **real production solver** (the unified/FMM path), not the Theta\*-only
  recorder. Delete the Theta\*-only `DijkstraEvent` recording once dead.
- SPA: a progress bar (from closest-to-goal %) + the live best-path polyline
  growing/refining toward the goal; optionally the top-k frontier branches as
  faint "candidate trails". Replaces the broken replay/live panel.
- **Gate:** plotting the 11.5 km route shows a moving bar + a trail forming
  within ~1 s and refining to the final route; no console errors.

### Phase 3 — Unify graph + mesh; delete legacy (the smartness fix)
Files: new `cost_field.rs` (surface raster), `elastica.rs`, `fmm_adapter.rs`,
`pathfinder.rs` (gut `solve_inner`), delete `core::off_trail`, `config.rs`.
- **Rasterise the trail graph into the corridor** as a surface-pace layer:
  for each cell, nearest trail within ½ cell ⇒ that trail's surface pace;
  else off-trail terrain pace. Reuse the graph's spatial index.
- **Single solve:** `solve_inner` becomes `corridor solve` for in-range
  routes; graph-Dijkstra (same cost model) only for `> max_corridor_km`.
  Delete `try_on_graph`/`try_hybrid`/`solve_off_trail` voting.
- **Trail-geometry snap:** post-process — runs of cells whose surface = a
  trail snap to that trail's exact polyline, so the drawn route follows the
  centreline, not a cell stairstep.
- **Delete:** Theta\* + mesh builder + hybrid + `CostMode::Multiplicative` +
  `off_trail_base` + (pending perf proof) the 2D FMM path.
- **Errors:** implement the honest-error table above; remove all silent
  fallbacks.
- **Gate:** Gamli-fjellet-type detours gone (detour metric drops); on-graph
  corpus quality unchanged (~99); water gate 0; timing budget held; a route
  through an isolated basin returns `Unreachable`, not a line.

### Phase 4 — Cleanup + docs
- Remove now-dead config knobs, types, tests tied to deleted paths.
- Update CLAUDE.md / module docs to describe the single solver.
- Full workspace test pass; `water_check --corpus` 60/60; corpus + timing
  baselines re-recorded.

---

## Verification matrix (run at every gate)

| Check | Tool | Pass condition |
|---|---|---|
| Water/glacier crossings | `water_check.py --corpus` | 0 hard crossings |
| Route quality | `terrain_metrics.py --force-off-trail` | composite ≥ 94.7, no metric regresses > a few pts unexplained |
| Detour-following | new cost-effectiveness metric | drops materially in Phase 3 |
| Speed | corpus per-route wall-clock | within budget (e.g. ≤ 5 s for ≤ 12 km) |
| Switchback / lake / refusal | `cargo test -p turbo-tiles-fmm` | all pass |
| Visual | `slope_render.py` + SPA shots | routes go around water, follow trails only while cheaper, look organic |

---

## Risks
- **Lifted A\* perf as the sole solver.** The 16× heading lift must be fast
  enough to retire the 2D FMM. Mitigation: A\* + adaptive cell + lazy cost;
  if a gentle route still over-explores, drop the heading lift adaptively
  where the grade cap never binds (detected per corridor) — a principled
  optimisation, not a second code path.
- **Trail rasterisation fidelity.** ½-cell snapping + polyline post-snap must
  not double-count or zig-zag at trail junctions. Covered by the visual gate.
- **Heuristic admissibility.** `min_pace` must be a true lower bound (fastest
  possible surface, downhill Tobler) or A\* loses optimality. Unit-test it.
