# Plan: Multi-waypoint route planning (route through an ordered list of points)

## Goal

Let the planner route through an **ordered list of points** `P0 → P1 → … → Pn`
(start, any number of intermediate "via" stops, end) instead of just `from → to`,
with first-class UX. Waypoints are **hard pass-through** points: the route MUST
visit each in order (the standard semantics in Komoot/Gaia/Google). The unified
solver stays untouched and atomic; multi-point is an orchestration layer on top.

## Decisions locked (2026-05-31)
- **Scope:** full feature — all 6 stages (incl. cumulative live preview,
  drag-the-line-to-insert, undo/redo, reverse).
- **Add gesture:** append = new end. Each empty-map click appends a point; the
  newest click is always the destination, earlier interior clicks become vias.

## Architecture principle

Keep `Pathfinder::solve(from, to, prefs) -> Path` as the **atomic primitive**.
A multi-waypoint route is N independent consecutive solves stitched into ONE
`Path`. This is the same shape the hybrid leg stitching already uses, costs
nothing in solver complexity, and means every quality property we validated
(trail-following, water-safety, live progress) holds per-segment automatically.

Each segment is solved independently, so the route may have a slight heading
kink at a via point — expected and standard. Documented, not a bug.

---

## Stage 1 — Backend: segment orchestration in the Pathfinder

**`crates/turbo-tiles-pathfind/src/pathfinder.rs`**

- Add `pub fn solve_route(&self, points: &[[f64; 2]], prefs: &Prefs) -> Result<Path, PathfindError>`:
  - Require `points.len() >= 2`; `< 2` → `DegenerateInputs`.
  - For each consecutive pair `(points[i], points[i+1])` call `self.solve(...)`.
  - **Stitch** the per-segment `Path`s into one:
    - `geometry`: append, dropping the duplicated shared vertex at each seam.
    - `legs`: extend, **re-offsetting** `start_idx`/`end_idx` into the merged
      geometry (the existing `PathLeg` indices are segment-local).
    - `length_m`, `cost`: sum.
    - `distances_m`: recompute as one cumulative array over the merged geometry.
    - `fkb_breakdown`: merge by key (sum metres per surface).
    - `on_trail_pct`: recompute weighted over total length.
    - `refused_by`: set-union.
    - `strategy`: `Hybrid` if segments differ, else the common one.
    - `recording`/`debug`: keep `None` here (record path handled in Stage 3).
  - **Error attribution**: wrap any segment error as
    `PathfindError::SegmentFailed { leg_index, from_point, to_point, source }`
    (new variant) so the UI can point at the exact failing waypoint pair.
- Add a `pub waypoint_legs: Vec<WaypointLeg>` field to `Path` (new struct:
  `{ from_idx, to_idx, length_m, cost, strategy, geometry_start_idx, geometry_end_idx }`).
  Single 2-point solves emit a one-element list, so callers have a uniform shape.
  `solve(from,to)` becomes `solve_route(&[from,to])` internally (one code path).

**Why in the Pathfinder, not the HTTP layer:** keeps the HTTP handler a thin
shim (it already is), makes `solve_route` unit-testable, and lets the
record/stream endpoints reuse it.

### Verification
- `crates/turbo-tiles-pathfind/tests/pathfinder.rs`: new tests on the synthetic
  graph/DEM fixtures —
  - 3-point route length ≈ sum of the two 2-point legs; geometry continuous
    (no duplicated seam vertex; `distances_m` monotonic).
  - `waypoint_legs` has `n-1` entries with correct index ranges.
  - A mid-route waypoint in a refused cell → `SegmentFailed { leg_index: k }`.
  - 2-point call is byte-identical to the old `solve` output (regression guard).

---

## Stage 2 — API: accept an ordered point list

**`crates/turbo-tiles-api/src/v1/pathfind.rs`**

- Extend `PathfindReq` to accept **either** shape (backward compatible):
  - existing `from`/`to`, OR
  - `points: Vec<[f64; 2]>` (≥ 2).
  Normalize to `points` server-side (`from`/`to` → `[from, to]`). Use
  `#[serde(default)]` + a small validator; reject "both and inconsistent".
- Handler calls `pf.solve_route(&points, &prefs)`; response unchanged except
  `Path` now carries `waypoint_legs`. Crash-dump req body includes `points`.
- Map `SegmentFailed` in `map_pathfind_err` to a `BadRequest`/`NoCoverage`-style
  error whose `details` include `leg_index` + the two points, so the SPA can
  highlight the offending stop.
- `pathfind_record` and `pathfind_stream`: same `points` normalization (no other
  change for `/record`). `/stream` handled in Stage 3.

### Verification
- `tools/route-scenarios.toml`: add a 3-point scenario (start → via → end) and
  extend the harness to post `points` when present. Assert total length range +
  that it passes through the via (min distance from route to the via ≈ 0).
- `curl` smoke: `{"points":[[lon,lat],[lon,lat],[lon,lat]],"profile":"foot"}`.

---

## Stage 3 — Backend: live progress across segments

**`pathfinder.rs` (solve_route) + `crates/turbo-tiles-api/src/v1/pathfind.rs`**

- The thread-local recorder is installed once around the whole `solve_route`
  (it already is, via `Pathfinder::solve`'s wrapper — move the recorder install
  up to `solve_route` so it spans all segments).
- For a cumulative preview, prefix each segment's `BestPathSnapshot` with the
  already-finalized geometry of prior segments, so the blue line grows through
  the via points rather than restarting at each one. Implement by having
  `solve_route` pass a "frozen prefix" to a thin wrapper that rewrites snapshot
  coords before `record()` (kept inside the record closure → zero cost when not
  recording). `/stream` then naturally streams the whole multi-leg build.
- Emit a `begin_phase("leg {i}")` per segment so the replay panel groups legs.

### Verification
- `/v1/pathfind/record` with 3 points → recording has snapshots whose final
  frame spans the full multi-leg route; phases named per leg.

---

## Stage 4 — SPA data model: ordered points

**`apps/admin/src/screens/PlotRoute.tsx`** + **`apps/admin/src/api/v1.ts`**

- Replace `from`/`to` state with `points: Marker[]` (`Marker = [lon, lat]`),
  `setPoints`, and a parallel `markersRef: maplibregl.Marker[]`.
- `api/v1.ts`: `pathfind`/`pathfindRecord`/`pathfindStream` accept
  `points: [number, number][]`; add `waypoint_legs` to the `Path` type.
- Map-click behavior:
  - click on empty map → **append** a point (becomes the new destination).
  - first point = start, last = end, middle = via.
- Debounced auto-recompute (existing pattern) fires on any `points` change with
  `len >= 2`.

---

## Stage 5 — SPA UX (the core of the ask; CVD-first because the user is colour-blind)

**Marker design — encode order by SHAPE + LABEL, never colour alone:**
- Start: rounded pin labeled **S** (CVD blue `#0072B2`).
- Via: numbered circles **1, 2, 3 …** (neutral grey ring + white fill, black
  number) — distinguishable by the number, not hue.
- End: square/flag pin labeled **E** (CVD vermillion `#D55E00`).
- Every marker has a visible integer order badge so the sequence reads without
  relying on colour.

**Waypoint list panel** (left/side panel, the command center):
- Ordered rows: `S → 1 → 2 → … → E`, each row shows the leg distance + time +
  ascent to the **next** point (from `waypoint_legs`), plus a running total at
  the bottom (total length / time / ascent / % on-trail).
- Per-row controls: drag handle (reorder), delete (✕), "center map here".
- "Reverse route" button; "Clear all" button.
- A failing leg row is flagged inline ("⚠ no route to stop 2") and its marker
  pulses, driven by the `SegmentFailed.leg_index` from the API.

**Direct map manipulation (great UX):**
- **Drag a marker** to move that waypoint → recompute (markers already draggable
  in MapLibre; wire `dragend` → update `points[i]`).
- **Drag the route line to insert a via** (the Google-Maps gesture): on
  `mousedown` over the route, create a provisional point that follows the
  cursor; on release, insert it into `points` at the index **between the two
  waypoints whose leg was grabbed** (determine via the `waypoint_legs` geometry
  index ranges → which leg the grabbed vertex belongs to). This is the single
  highest-leverage UX touch; spec it now, implement behind the same recompute.
- **Insert mode**: clicking an existing route also offers "add stop here".
- **Delete**: click a marker → small popover with "remove stop".

**Editing affordances:**
- Undo/redo stack for point edits (add/move/delete/reorder).
- Keyboard: `Esc` cancels an in-progress drag/insert; `Backspace`/`Delete`
  removes the last/selected stop; number keys focus a stop in the list.
- Empty state coaching: "Click the map to drop a start, then keep clicking to
  add stops. Drag the line to insert a stop between two others."

**Rendering:**
- Reuse the existing per-leg renderer (blue on-trail / vermillion off-trail).
  Optionally draw thin neutral tick marks at via points so leg seams read.
- Live preview (Stage 3) grows through the whole multi-leg route.

### Verification (SPA)
- `apps/admin/shot-*.mjs` Playwright: script a 3-stop route (click ×3), assert 3
  markers (S/1/E labels), a stitched route, and a non-empty `waypoint_legs`
  summary; drag the middle marker and assert recompute; drag the line to insert
  a 4th stop. Screenshot for visual confirmation.

---

## Stage 6 — Docs, scenario lock-in, memory

- Update `docs/unified-routing-2026-05-30.md` (or a short addendum) with the
  multi-waypoint orchestration contract (atomic solve + stitch + per-leg errors).
- Lock the 3-point scenario in `tools/route-scenarios.toml`.
- Memory: note that multi-waypoint = per-segment solve + stitch, hard
  pass-through semantics, errors attributed by `leg_index`.

---

## Critical files

| Concern | File |
|---|---|
| Segment orchestration + stitch + `solve_route` + `WaypointLeg` + `SegmentFailed` | `crates/turbo-tiles-pathfind/src/pathfinder.rs` |
| API: `points` request, error attribution | `crates/turbo-tiles-api/src/v1/pathfind.rs` |
| Live progress across legs | `pathfinder.rs` (recorder span) + `pathfind.rs` (`/stream`) |
| SPA state, markers, list panel, drag-to-insert, CVD markers | `apps/admin/src/screens/PlotRoute.tsx` |
| SPA API client types | `apps/admin/src/api/v1.ts` |
| Tests | `crates/turbo-tiles-pathfind/tests/pathfinder.rs`, `tools/route-scenarios.toml`, `apps/admin/shot-*.mjs` |

## Verification gate (whole feature)
```
cargo test -p turbo-tiles-pathfind            # solve_route stitch + per-leg error
cargo build --release --bin tileserver
./tools/dev-serve-full.sh                      # + cd apps/admin && npm run dev
cargo test --workspace --test scenarios        # incl. 3-point scenario
# Playwright: 3-stop route, drag-move, drag-insert, screenshot
```
**Pass criteria:** N-point routes stitch into one continuous `Path` that passes
through every stop; per-leg + total stats correct; a bad stop fails with the
exact `leg_index`; 2-point routes are unchanged (regression); live preview grows
through all legs; markers are order-readable without colour.

## Sequencing / shipping
- **MVP (Stages 1–2, 4, core of 5):** append-click points, numbered CVD markers,
  waypoint list with per-leg stats, drag-to-move, per-leg errors. Fully usable.
- **Polish (Stage 3 + drag-to-insert + undo/redo):** cumulative live preview,
  Google-style line-drag insert, undo/redo, reverse.

## Risks & mitigations
- **Per-segment kinks at via points** — expected; document. (A future
  heading-aware stitch is out of scope.)
- **N solves = N× latency** — segments are independent; if needed, solve them
  concurrently with `spawn_blocking` + join in the API handler (note: breaks
  in-order streaming, so keep sequential while `record`/`stream` is on).
- **Drag-to-insert index math** — derive the insert index from `waypoint_legs`
  geometry ranges, not from nearest marker, so it's unambiguous which leg was
  grabbed. Covered by a Playwright assertion.
- **Backward compatibility** — `from`/`to` requests keep working via
  normalization; a regression test pins identical 2-point output.

## Out of scope
- Optimal stop **reordering** (TSP) — waypoints stay in user order.
- Round-trip / loop generation.
- Per-waypoint dwell times or scheduling.
- Heading-continuous stitching across via points.
