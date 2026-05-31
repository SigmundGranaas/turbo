# Off-trail routing — performance, live progress, and route-quality analysis

_2026-05-30. Grounded in the current code + a measured 11.5 km route
(`14.73394,67.28833 → 14.87911,67.37537`)._

Three distinct problems, three distinct root causes. Treated together
because the proper end-state (a single unified solve) fixes all three.

---

## 1. Performance — long off-trail routes take minutes

**Measured (11.5 km straight-line, force-off-trail):**

| solver | wall time | points |
|---|---|---|
| 2D anisotropic FMM (`grade_limited` off) | **49.7 s** | 192 |
| grade-limited lifted (`grade_limited` on) | **160.7 s** | 13 536 |

### Root causes

1. **Search corridor area grows O(distance²).** `fmm_adapter::compute_corridor_shape`:
   `pad = max(40 m, 0.30·d)` and `half_width = max(800 m, 0.20·d)` both scale
   with route length. For 11.5 km that's an ~18 × 11.5 km rectangle whose
   axis-aligned bbox is **~21 × 20 km → 4.4 M cells @ 10 m**. Double the
   length ⇒ 4× the work. This alone explains the 49.7 s *2D* baseline.
2. **The grade-limited solver multiplies that by 16.** It lifts to
   `(x, y, heading)` with `N_HEADINGS = 16` → **~71 M states**. Per request it
   allocates + fills `arrival` (f32 ≈ 284 MB) + `parent` (u32 ≈ 284 MB) +
   `accepted` (bool ≈ 71 MB) ≈ **640 MB** before doing any work.
3. **It's Dijkstra — it floods.** `solve_lifted_grade_limited` expands states
   in pure cost order with **no goal bias**, so it explores most of the
   corridor before reaching the goal.
4. **The contributor cost overlay is baked eagerly over every cell.** The
   water-safety fix (`solve_grade_limited_path`) loops over all 4.4 M cells
   calling the water shoreline spoke-test + trail-proximity rstar query +
   DEM-sampling slope contributor on each — including cells never visited.
5. **10 m cells** — 4× the cell count of a 20 m grid.

### Fixes (cheapest-first)

| # | Change | Expected win | Effort |
|---|---|---|---|
| P1 | **A\*** — admissible heuristic (`remaining_straight_line × min_pace`) added to the heap key so the solver beelines to the goal instead of flooding | **5–30× fewer states** (biggest lever) | Medium, self-contained |
| P2 | **Cap the corridor** (`pad ≤ ~1.2 km`, `half_width ≤ ~1.2 km`) → area O(d) | 3–5× fewer cells + much less memory | Small |
| P3 | **Adaptive cell size** (10 m short routes, 20 m long) | ~4× fewer states | Small |
| P4 | **Cheap/lazy cost**: read water refusal from the mask raster (O(1)) instead of the spoke-test; bake cost lazily only for A\*-visited cells | removes the eager all-cell bake | Medium |

Target: 160 s → low single-digit seconds.

---

## 2. Live progress — the preview is blank, and we want to *see* trails forming

### Why it's blank today
The recording / SSE-streaming path (`/v1/pathfind/stream`,
`pathfinder.rs::graph_observer`) is wired **only into the legacy Theta\* /
graph Dijkstra** (it records `DijkstraEvent`s). The **FMM and grade-limited
solvers emit no events at all.** Force-off-trail uses the grade-limited
solver → nothing streams → the user stares at a spinner for ~2.7 min.

### What to build (requirement: watch candidate trail(s) grow toward the goal)
- Thread an optional `on_progress` callback into `solve_lifted_grade_limited`
  (and the 2D FMM), invoked every ~50 k pops.
- Each tick: reconstruct the **best path so far** by backtracking `parent`
  from the lowest-`g+h` frontier state, and emit it as a `BestPathSnapshot`
  (plus explored count + closest-to-goal distance for a % bar).
- Stream those over the existing SSE channel; the SPA already renders a
  `replay-best` polyline — repoint it at live snapshots so the user sees one
  (or several, if we keep the top-k frontier branches) tentative trail
  **reaching out and refining toward the goal** as the solve runs.
- A\* makes this naturally legible: the snapshot path marches steadily
  goalward instead of a Dijkstra blob.

---

## 3. Route quality — it follows trail detours past the point of being worth it

Observed: a route rides a marked trail through a detour instead of leaving it
where cutting across would be cheaper (Gamli-fjellet area).

### What's actually happening
Strategy selection is **already cost-based** — `solve_inner` computes
`on_graph`, `hybrid`, and `off_trail` and returns the cheapest by cost. It is
**not** naive "first viable." The detour-following comes from three real
limits:

1. **`off_trail_base = 2.3×` (foot) makes trails very sticky.** Off-trail
   metres cost 2.3× trail metres, so the solver prefers a trail until the
   off-trail shortcut would be shorter than ~1/2.3 ≈ **43%** of the trail
   distance. In effect it will follow a trail detour up to ~2.3× the
   cross-country distance — far stickier than a real hiker, who leaves a
   detour that adds even ~30–50%.
2. **Whole-route strategy choice — no mid-route trail-leaving.** It's
   all-on-graph **or** all-off-trail **or** hybrid, and **hybrid only bridges
   off-trail at the two endpoints** (graph in the middle). There is no way to
   express "ride the trail, cut across this one detour in the middle, rejoin
   the trail." So when the trail is right for 90% of the route, the solver
   takes the whole trail — detour included — because the only off-trail
   alternative throws away all the trail benefit.
3. **Long routes skip off-trail entirely.** `max_off_trail_km = 10`, so any
   route longer than that never even computes an off-trail candidate → it can
   only return a graph path, detours and all.

### Fixes
- **Interim (tuning):** lower `off_trail_base` (e.g. 1.5–1.8) and/or make it
  terrain-aware so trails stop being worth a 2.3× detour. Cheap, partial.
- **Proper (structural): a single unified graph + mesh solve.** Embed the
  trail graph as **cheap edges inside the terrain cost field** (mesh / lifted
  lattice) and run **one** shortest-path over the combination. The optimal
  path then rides trails exactly where they're cheaper and leaves them the
  instant a cross-country step costs less — "follow paths only while
  cost-effective," evaluated at **every point**, not as a whole-route choice.
  This **subsumes the hybrid bridge** (#158), removes the `off_trail_base`
  hack, and removes the long-route off-trail skip.

---

## Recommended roadmap (as a whole)

1. **Speed now** — P2 (corridor cap) + P3 (adaptive cell) + **P1 (A\*)**.
   Turns the 160 s route into seconds. Verify each against `water_check.py`
   + the corpus so speed doesn't reintroduce lake-crossing.
2. **Progress** — emit best-path-so-far + explored stats from the solver and
   stream them; SPA draws the growing candidate trail(s). Fixes the blank
   preview.
3. **Smartness (the big one)** — unified graph+mesh single-solve. Fixes
   detour-following structurally, subsumes hybrid, and the A\* + cost field
   from steps 1–2 are exactly the machinery it needs. Ship the
   `off_trail_base` reduction as an interim mitigation first.

All three converge on the same end-state: **one A\*-driven solve over a single
cost field that contains both terrain and trails**, with a progress callback
that streams the emerging path.
