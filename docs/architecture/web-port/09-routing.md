# 09 — Route planning

> A unified Create-Route tool with three modes — **Route** (snap-to-trail
> pathfinder with SSE live preview), **Line** (manual waypoint polyline), and
> **Draw** (freehand stroke) — that produces a saveable path.

## Status
- **Android (gold standard):** A single Create-Route tool with a 3-way pill
  toggle `TrackMode { Route, Line, Draw }` (`CreateTrack.kt`, `MapScreen.kt`,
  `RouteViewModel.kt`).
  - **Route:** tap origin → tap destination → the solver streams over SSE and the
    geometry live-previews as it computes. Waypoints are ordered (index 0 =
    origin, last = destination, middle = vias). Add a stop by sequential tap
    (append to end → old dest becomes a via), from a marker, or **insert at
    least-detour** position (`Waypoints.insertLeastDetour`). Reorder in a sheet
    (`WaypointsSheet`, explicit ↑/↓). Move a waypoint by dragging it on the map
    (re-solve debounced 300 ms and **deferred to drag-end** so the line doesn't
    thrash). Remove via long-press or the sheet (endpoints can't be removed; route
    clears when <2 remain). Re-solve keeps the **old geometry on screen** instead
    of snapping to a straight line (only the first solve shows straight→refined).
  - **Presets:** `RoutePreset { Balanced, AvoidRoads, Direct, EasyGrade,
    TrailPurist }` (hardcoded enum with key/label/description; the app does **not**
    fetch `/presets`). Profile `foot` (default) / `bicycle` / `ski`.
  - **Stats:** distance, duration (Naismith), ascent, on-trail %, surface
    breakdown bar (`trail` / `road` / `ski_track` / `off_trail` / `unknown`).
  - **Line:** each tap appends a waypoint; geometry is the polyline, built
    client-side (no solver).
  - **Draw:** a full-screen drag overlay samples the finger stroke into LatLng
    points (no solver).
  - **Save:** name dialog (default `"Track <distance>"`) → `SavedPath` (Route mode
    → `source = Route`; Line/Draw → `source = Measure`) via `PathRepository`, then
    an optional "Saved · Follow" snackbar.
- **Web today:** Not implemented. No route tool, no SSE client, no waypoint
  editing.
- **Renderer/back-end prerequisites:**
  - `POST /api/route/plan/stream` (SSE; the primary endpoint — Android uses only
    this), `POST /api/route/plan` (blocking; optional for the web port),
    `GET /api/route/presets`. All **public/unauthenticated** (do not add auth).
  - The managed API gateway is a transparent proxy: `/api/route/*` →
    tileserver `/v1/route/*`.
  - **Renderer:** `unproject(x,y)` / `project(lat,lng)` for map taps + waypoint
    hit-testing, `apply_scene` to update the previewed geometry each SSE chunk,
    `ease_to` to frame. Optional raised tube needs `set_route_tube` exposed in
    `turbomap-web` (engine already implements it) — flat `line` layer otherwise.

## User stories

### 1. Plan a route between two points
*As a user, I want to tap a start and an end and get a snapped trail route, so that I can plan a hike/ride/ski.*

**Acceptance criteria**
- Entering **Route** mode, the first map tap sets the origin; the second tap sets
  the destination and triggers a solve.
- The solve calls `POST /api/route/plan/stream` with body
  `{ points:[[lon,lat],…], preset, profile }` and `Accept: text/event-stream`.
- The resulting geometry (a `LineString`) is drawn on the map and stats are shown.
- A long-press map menu offers "Route here" (origin = current GPS location → tap)
  and "Start route here" (set origin) as shortcuts (parity with Android).

**Web-specific notes**
- Map taps convert screen px → LatLng via `unproject`. Wire coords are
  `[lon, lat]`; convert at the boundary (`unproject` returns lat/lng).

### 2. Watch the live SSE preview
*As a user, I want to see the route refine in real time while the solver works, so that planning feels responsive.*

**Acceptance criteria**
- The stream is read with `fetch` + `ReadableStream` (the endpoint is a POST, so
  `EventSource` cannot be used). A `TextDecoder` + line buffer parses standard SSE
  framing (`event:` / `data:` lines, blank line ends a frame; ignore `:` keep-alive
  comments sent every ~15 s).
- Three event types are handled:
  - `event: progress`, `data: { coordinates:[[lon,lat],…] }` — a **best-path-so-far
    snapshot**; it **fully replaces** the current preview (not incremental). Each
    snapshot rebuilds the `geo-json` overlay and calls `apply_scene`.
  - `event: result`, `data: <full RoutePlanResp>` — terminal success; final
    geometry + stats applied.
  - `event: error`, `data: { error }` — terminal failure.
- Stream shape is `progress* (result | error)`. Exactly one terminal event.
- On the **first** solve the preview animates from a coarse path to the refined
  one; on a **re-solve** where a route already exists, intermediate `progress`
  snapshots are ignored and the old geometry stays drawn until the new `result`
  arrives (no straight-line flash — parity with Android).

**Web-specific notes**
- Each new solve aborts the prior stream via `AbortController` (mirrors Android's
  job cancel).
- Throttle `apply_scene` to animation frames if snapshots arrive faster than the
  render loop.

### 3. Switch presets and profile
*As a user, I want to choose a routing style and activity profile, so that the route suits my trip.*

**Acceptance criteria**
- A preset picker offers **Balanced** (default), **Avoid roads**, **Direct**,
  **Easy grade**, **Trail purist**; a profile control offers **foot** (default) /
  **bicycle** / **ski**.
- Presets/labels/descriptions are fetched from `GET /api/route/presets`
  (`[{ name, label, description }]`) so the list stays dynamic; the five `name`
  values are stable + OpenAPI-validated, so a hardcoded fallback is acceptable if
  the fetch fails.
- Changing the preset or profile re-solves immediately (not debounced) using the
  current waypoints, keeping the old geometry on screen until the new result lands.

### 4. Add, reorder, move, and remove stops
*As a user, I want to add intermediate stops and rearrange them, so that I can shape the route.*

**Acceptance criteria**
- **Add (sequential tap):** when a 2+-point route exists, a map tap appends a
  waypoint to the end (former destination becomes a via).
- **Add (least-detour):** an "Add stop" action inserts the next tapped point at the
  position minimizing total added distance (haversine; endpoints never displaced).
- **Add from marker:** a marker's detail can "add as stop" (least-detour insert).
- **Reorder:** a stops sheet lists origin → vias → destination with ↑/↓ controls;
  reordering re-solves.
- **Move on map:** tapping a waypoint selects it; the next map tap (or a drag)
  relocates it. Dragging defers the re-solve to **drag-end** (`beginWaypointDrag` /
  `endWaypointDrag` equivalent); a non-drag relocate re-solves debounced **300 ms**.
- **Remove:** long-press a waypoint or use the sheet (vias only; origin/destination
  fixed). Removing until <2 remain clears the route.
- An **undo** stack (≥20 entries) reverts the last waypoint edit.

**Web-specific notes**
- Waypoint handles are HTML/DOM markers positioned via `project(lat,lng)` each frame
  (or rendered as a `circle` layer + `hit_test`); drag uses pointer events with a
  debounce, and the live drag updates the handle position without re-solving until
  pointer-up.

### 5. Draw a freehand line
*As a user, I want to sketch a line with my finger/mouse, so that I can capture an arbitrary path without the solver.*

**Acceptance criteria**
- **Draw** mode shows a full-canvas capture overlay; pointer-down clears the
  current stroke, each pointer-move sample is converted via `unproject` and
  appended, pointer-up ends the stroke.
- Geometry is the sampled polyline, built entirely client-side (no API call);
  distance is computed locally (haversine).
- Samples are thinned (min pixel/meter spacing) to keep the polyline manageable.

**Web-specific notes**
- The overlay must not let the gesture pan the map while drawing; pointer capture is
  taken on pointer-down and released on pointer-up.

### 6. Build a manual waypoint line
*As a user, I want to place waypoints one tap at a time to form a straight-segment line, so that I can measure or plan without snapping.*

**Acceptance criteria**
- **Line** mode appends a waypoint on each tap; the geometry is the straight-segment
  polyline through them, built client-side (no solver).
- Distance updates live as points are added; the last point can be undone.

### 7. See route stats
*As a user, I want distance, time, ascent, on-trail %, and surface mix, so that I can judge the route.*

**Acceptance criteria**
- For **Route** mode, stats come from the SSE `result` (`RoutePlanResp`):
  `distance_m`, `duration_s` (Naismith), `ascent_m`, `on_trail_pct`, and
  `surfaces { trail, road, ski_track, off_trail, unknown }` (meters; only present
  keys included).
- A proportional **surface-mix bar** + legend renders the surface breakdown (color
  map: trail → primary, road → outline, off-trail → orange, etc.).
- For **Line/Draw**, only distance (and optionally a backfilled ascent via the
  elevation profile endpoint, doc 08) is shown — no surface/on-trail data (no
  solver ran).
- Duration/distance are formatted per the user's units (doc 19).

### 8. Save a route as a track
*As a user, I want to save my planned route/line as a track, so that I can revisit or follow it.*

**Acceptance criteria**
- A **Save** action is enabled when Route mode has a completed result, or Line/Draw
  has ≥2 points.
- Save opens a name dialog (default `"Track <distance>"`), then `POST
  /api/tracks/tracks` with the geometry, computed stats, and a client-side
  `source` (Route → "Route"; Line/Draw → "Drawn"/Measure) stored locally (doc 08 —
  backend does not persist source).
- After save, a "Saved · Follow" affordance offers to start following immediately
  (doc 11).
- The saved track appears in the Paths list (doc 08).

**Web-specific notes**
- Requires sign-in (the tracks endpoint is auth-only); an unauthenticated user is
  prompted to sign in before save (the route can still be planned/previewed
  without an account since routing is public).

## Primary flows (web)

**Plan + save (happy path):** open Create-Route → Route mode → tap origin → tap
destination → SSE stream opens, `progress` snapshots progressively replace the
previewed line, then `result` finalizes geometry + stats → adjust preset (re-solve)
→ add a via (least-detour, re-solve) → drag a waypoint (re-solve on drop) → Save →
name → `POST /api/tracks/tracks` → "Follow?".

**Draw:** Draw mode → drag a stroke → release → distance shown → Save.

**Edge cases:**
- **Solver error / no route found:** `event: error` (or non-2xx) → show "No route
  found / routing failed", keep waypoints, let the user adjust and retry.
- **Solver timeout:** if no terminal event within a client deadline, abort the
  stream and surface a timeout with retry.
- **Network drop mid-stream:** the `ReadableStream` read rejects → keep the last
  `progress` preview, mark it "preview (interrupted)", and offer retry; never leave
  a half-applied geometry as final.
- **Single point / no destination:** Save disabled; tool prompts for the next tap.
- **Unauthenticated save:** plan works; Save prompts sign-in.

## UI / UX on web
- The Create-Route tool is a **bottom sheet** (mobile) / **side panel** (desktop)
  with a mode pill toggle at the top (Route / Line / Draw), the hero distance stat,
  the surface-mix bar (Route only), the preset + profile row (Route only), a stops
  row (Route, shown when ≥2 stops), and an action bar (undo / clear / Save /
  Follow).
- The stops sheet (manage waypoints) is a secondary sheet with origin/via/dest rows
  and ↑/↓ reorder + remove (vias).
- The sheet height is reported via `set_viewport_inset` so route framing and tap
  unprojection account for the obscured area.
- Desktop uses pointer drag for waypoint move + Draw; touch uses the same pointer
  events. A long-press (touch) / right-click or modifier-click (desktop) opens the
  map context menu (Route here / Start route here / Create track).

## Data & APIs
- `POST /api/route/plan/stream` (public, SSE) — **primary.** Body
  `{ points:[[lon,lat],…] (≥2), preset?, profile? }`, `Accept: text/event-stream`.
  Events: `progress { coordinates }`, `result <RoutePlanResp>`, `error { error }`.
- `POST /api/route/plan` (public) — blocking variant; same body, returns
  `RoutePlanResp` directly. Optional for the web port (e.g. non-streaming fallback).
- `GET /api/route/presets` (public) — `[{ name, label, description }]`.
- `RoutePlanResp`: `{ distance_m, duration_s, ascent_m, on_trail_pct,
  surfaces:{[key]:meters}, geometry:{ type:"LineString", coordinates:[[lon,lat]] },
  legs:[{ from_index, to_index, distance_m }] }`.
- `preset ∈ { balanced(default), avoid_roads, direct, easy_grade, trail_purist }`;
  `profile ∈ { foot(default), bicycle, ski }`.
- Save uses the Tracks API (doc 08): `POST /api/tracks/tracks` (auth).
- **TanStack Query keys:** `['route','presets']` (cached presets). The streamed
  plan is **not** a Query (it's an imperative SSE side-effect driving Zustand) — use
  a mutation/manual fetch with `AbortController`.
- **Zustand slices:** `route` slice — `mode` (Route/Line/Draw), `waypoints`
  (ordered), `preset`, `profile`, `solveState` (idle/solving/done/error),
  `previewGeometry`, `stats`, `undoStack`, `selectedWaypoint`, `dragActive`,
  `linePoints`, `drawPoints`.

## Renderer integration
- **Preview overlay:** a `geo-json` source + `line` layer holding the current
  geometry; each `progress`/`result`/local edit rebuilds the source and calls
  `apply_scene`. Waypoints render as a `circle`/`symbol` layer (or DOM markers via
  `project`).
- **Taps & drags:** `unproject(x,y)` for placing/moving points and reading the Draw
  stroke; `project(lat,lng)` to position DOM waypoint handles; `hit_test` (if
  exposed) for waypoint pick — otherwise nearest-point in screen space.
- **Framing:** `ease_to` to route bounds after a result, honoring
  `set_viewport_inset`.
- **Optional tube:** raised route tube needs **`set_route_tube` exposed in
  `turbomap-web`** (thin wasm-bindgen passthrough; engine already implements it);
  flat `line` layer is the baseline.
- Coordinate order: API geometry is `[lon, lat]`; the scene/`geo-json` overlay is
  also `[lng, lat]`; engine camera calls take `(lat, lng)` — convert at the
  boundary.

## Out of scope (this phase)
- "Download along route" offline tile prefetch (offline/PWA phase).
- Turn-by-turn following, ETA, checkpoints, reroute — see doc 11 (this doc ends at
  Save + hand-off).
- Server-side persistence of route `source`/preset metadata on the track (kept
  client-side, doc 08).

## Open questions
- Should the web port fetch `GET /api/route/presets` for dynamic labels/icons, or
  hardcode the five stable presets like Android (with the fetch as enhancement)?
- Client-side solve timeout value before aborting a stalled stream?
- For Line/Draw, should ascent be backfilled via the elevation profile endpoint at
  save time (depends on the doc 08 backfill endpoint existing)?
- Waypoint hit-testing: rely on DOM markers + `project`, or expose `hit_test` in
  `turbomap-web` for engine-side picking?
