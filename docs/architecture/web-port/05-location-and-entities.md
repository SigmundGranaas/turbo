# 05 — Location & on-map entities

> Live user location (blue dot + heading beam), camera persistence/restore, and
> the engine-rendered on-map entities (route tubes, tracks, checkpoints, markers,
> waypoint/origin/my-position pins) — plus the shared **overlay compositor** that
> every later feature doc builds on for rendering.

## Status
- **Android (gold standard):**
  - **Live location:** `AndroidLocationRepository.samples()` emits
    `LocationSample(position, altitude, accuracyM, speedMps, bearingDeg?)` from the
    platform `LocationManager` (GPS + network), seeded with last-known so the dot
    shows immediately; filtered for stale/inaccurate fixes.
  - **My-position pin + heading beam:** rendered as a **Compose overlay**
    (`MyPositionPin` in `core/designsystem/.../MapOverlay.kt`), a white-haloed blue
    disc (`UserBlue = 0xFF1A73E8`) **projected through the engine seam**
    (`engine.toScreen(latLng)`), not a scene layer. When `userHeading` (course over
    ground, 0°=N) is non-null, a translucent gradient triangle beam is drawn; its
    angle is derived in **screen space** by projecting two points (the fix + a point
    ~20m ahead along the bearing), so it rotates correctly under map bearing/tilt.
    Off-screen, a chevron clamps to the viewport edge.
  - **Camera persistence:** `MapViewModel.saveCamera(lat,lng,zoom)` →
    `SettingsRepository.setLastCamera()` writes DataStore keys `last_camera_lat`,
    `last_camera_lng`, `last_camera_zoom`. Saving is **debounced**: a 1500ms loop in
    `MapScreen.kt` writes only on change, skipping null-island and the world-overview
    fallback, plus a save on `ON_PAUSE`. On launch the map waits for
    `settingsLoaded` then restores. **First launch (no saved camera):** centers on
    the first GPS fix at zoom 13; world-overview fallback if no fix arrives. A
    one-shot `didInitialCenter` flag prevents fighting manual pans.
  - **On-map entities rendered by the engine / overlay:** route tubes (bright
    ahead, dim walked, track), measure dots/line (geojson scene layers), checkpoints,
    markers, waypoint pins, route-origin pin, photo pins, my-position pin. Pins/
    markers/checkpoints are **Compose overlays projected via the engine seam**;
    **route/track tubes are raised 3D meshes** pushed imperatively via
    `nativeSetRouteTube` → `Cmd::SetRouteTube` → `engine.set_route_tube(id, points,
    color, radius)` (the tube does **not** go through the Scene JSON).
- **Web today:** Not implemented. `TurboMapCanvas.tsx` mounts with a hardcoded
  `DEFAULT_CAMERA` (Bergen), no `navigator.geolocation`, no camera save/restore, and
  builds only a base Scene. No entity overlays. The Scene IR `Layer` union supports
  only `raster | hillshade`.
- **Renderer/back-end prerequisites:**
  - **Widen the web Scene IR `Layer` union** in `apps/web/src/map/scene.ts` to add
    `line`, `fill`, `symbol`, `circle` (mirroring `turbomap-scene`; the engine
    already renders them). Needed for tracks, checkpoints, markers, waypoint/origin
    pins, and the my-position circle option.
  - **Expose `set_route_tube` in `turbomap-web`** — the engine method
    (`engine.set_route_tube(id, &points, color, radius)`) and core
    (`map.rs::set_route_tube`) already exist; only a thin wasm-bindgen passthrough is
    missing (`turbomap-web/src/lib.rs`), after which `wasm-pack` regenerates the
    `.d.ts`. Required for raised route/track tubes (used by routing, recording,
    follow docs). **Reconcile the radius param** (Android JNI marshals `radius_m`
    while core expects `radius_px` — web should pass pixel radius).
  - **Projection** already exists in the wrapper: `project(lat,lng)` /
    `unproject(x,y)` and `camera_json()` — used for DOM/Canvas pin overlays and for
    deriving the heading beam angle.
  - Live location uses `navigator.geolocation.watchPosition` (foreground only).

## User stories

### 1. See my location and heading
*As a user, I want a blue dot at my GPS position with a heading beam, so that I
know where I am and which way I'm facing.*

**Acceptance criteria**
- After granting geolocation permission, a blue, white-haloed dot appears at the
  current fix and updates as `watchPosition` emits.
- When the fix carries a `heading` (course over ground), a beam points that way;
  the beam rotates correctly when the map is rotated/tilted (3D), because its angle
  is computed from two **projected** screen points (fix + a point ahead on the
  bearing), matching Android.
- When the position is off-screen, an edge chevron indicates its direction; tapping
  recenters (story 2).
- A faint accuracy ring may scale with `coords.accuracy` (optional, parity-nice).

**Web-specific notes**
- The pin is implemented as a **DOM/Canvas overlay** positioned via
  `map.project(lat,lng)` each frame (the "Compose-equivalent overlay"), OR as a
  `geo-json` + `circle` layer in the Scene. Default to the **DOM overlay** for the
  animated dot/beam (cheap, crisp, easy hit-target), reserving the circle-layer path
  for when terrain-draping of the dot matters.
- **Background GPS is unreliable** when the tab is hidden — location updates run
  "while the app is open + screen on" (README constraint). No lock-screen/background
  fix like Android's service.

### 2. Recenter-on-me
*As a user, I want a button that recenters the map on my location, so that I can
get back to myself after panning.*

**Acceptance criteria**
- A recenter button is visible once a fix exists; it `ease_to`s the camera to the
  current position (keeping current zoom, or a sensible default if zoomed out far).
- Pressing it while a fix is pending shows a brief "locating…" state; if permission
  is denied, it opens the permission-help affordance instead of silently failing.
- Recentering does not lock the camera to follow — it is a one-shot ease (follow
  mode is doc 11).

**Web-specific notes**
- Mirrors Android's `didInitialCenter`/recenter behavior: manual pans are never
  overridden except by an explicit recenter or first-fix center.

### 3. The map remembers where I left it
*As a returning user, I want the map to reopen at my last position/zoom (and tilt/
bearing), so that I continue where I left off.*

**Acceptance criteria**
- Camera (`lat,lng,zoom`, and `pitch,bearing` for 3D) is persisted **debounced**
  (~1.5s on change) and on tab `visibilitychange`→hidden / `pagehide`.
- On load, the saved camera is restored **before** the first render so there's no
  visible jump from a default location.
- Saves skip null-island `(0,0)` and the world-overview fallback, mirroring Android.
- **First launch (no saved camera):** the map centers on the first GPS fix at a
  reasonable zoom (≈13); if no fix arrives (denied/no-fix), it stays at the
  world-overview fallback.

**Web-specific notes**
- Persist to **`localStorage`** (key e.g. `turbo.camera`); optionally also to server
  settings (doc 19) so it follows the account across devices — server sync is a
  nice-to-have, `localStorage` is the baseline. Read `camera_json()` for the full
  `{lat,lng,zoom,pitch,bearing}` snapshot when saving.

### 4. Entities draw correctly in 2D and 3D
*As a user, I want routes, tracks, checkpoints, markers, and pins to render
accurately whether the map is flat or tilted, so that the map is trustworthy.*

**Acceptance criteria**
- Geometry entities (tracks, route lines, checkpoints, markers, waypoint/origin
  pins) render via Scene `geo-json` sources + `line/circle/symbol` layers and stay
  glued to their coordinates when panning, zooming, rotating, and tilting.
- Raised **route/track tubes** render via `set_route_tube` and drape over terrain in
  3D (matching Android's tube look), not as flat lines.
- Pin overlays implemented as DOM/Canvas reproject every camera change (re-`project`
  on each frame/`tick`), so they don't lag the map.
- Toggling 3D (doc 02) does not drop or misplace entities.

**Web-specific notes**
- DOM pin overlays must reproject on every camera change (analogous to Android's
  `cameraTick`). The rAF loop already runs each frame, so the overlay layer reads
  `project()` for each visible entity per frame (or only when `camera_json()`
  changes, to save work).

## Primary flows (web)

**First launch (no saved camera)**
1. App mounts the map at the world-overview fallback (no flash of arbitrary city).
2. Browser prompts for geolocation (only when location is needed — no upfront gate).
3. On the first `watchPosition` fix, the camera `ease_to`s to the fix at zoom ≈13
   (one-shot); the blue dot appears.
4. Subsequent moves update the dot; the camera is not auto-followed.

**Returning launch (saved camera)**
1. On mount, read `localStorage` camera and pass it to `TurboMap.create` (or
   `set_camera` immediately) before the first render.
2. The debounced saver starts; geolocation still initializes for the dot but does
   **not** recenter (saved camera wins).

**Permission denied**
1. `watchPosition` error callback fires with `PERMISSION_DENIED`.
2. No dot is shown; the recenter button switches to a "location blocked" state that
   opens a short help popover (how to re-enable in the browser).
3. The map remains fully usable; camera persistence still works from manual pans.

**No fix / timeout**
1. Permission granted but no position yet (indoors, cold start). Show a subtle
   "locating…" indicator near the recenter button.
2. On timeout, keep waiting (watch stays active); do not center the camera. If this
   is first launch, remain on the world-overview fallback.

**Entity render (e.g. a planned route preview)**
1. A feature (routing/recording/follow) hands the compositor a set of geo-json
   overlays (route line, waypoints, origin pin) + any tube requests.
2. The compositor assembles one Scene (base + terrain + active map overlays from
   doc 04 + these entity geo-json sources/layers) and calls `apply_scene` once.
3. For raised tubes, it additionally calls `set_route_tube(id, points, color,
   radiusPx)` for the route/track and a dimmer one for the walked segment.

## UI / UX on web
- **My-position dot + beam:** a DOM/Canvas overlay layer absolutely positioned over
  the canvas, reprojected each frame.
- **Recenter button:** bottom-right map-overlay control cluster; states: idle /
  locating / blocked.
- **Permission help:** lightweight popover, not a blocking modal (browsers gate per
  API; no Android-style onboarding permission screen).
- **Composition with bottom sheets:** when a feature sheet is open, `set_viewport_inset`
  keeps the dot/centered content above it; the recenter target accounts for the
  inset.
- **Responsive:** desktop pointer hover on controls; touch-sized hit targets on
  mobile web.

## Data & APIs
- **Live location:** `navigator.geolocation.watchPosition(success, error,
  { enableHighAccuracy: true, maximumAge, timeout })`. Map
  `GeolocationCoordinates` → an internal `LocationSample`-equivalent
  (`{ lat, lng, accuracy, heading?, speed? }`). `heading` is `NaN`/null when the
  device can't supply course over ground → no beam (matches Android `bearingDeg?`).
- **Camera persistence:** `localStorage["turbo.camera"]` = JSON of `camera_json()`
  (`{lat,lng,zoom,pitch,bearing}`). Optional server mirror via settings (doc 19,
  auth required). No dedicated backend endpoint required for the baseline.
- **No new map data endpoints** — entity geometry comes from the feature docs
  (markers `/api/geo/locations`, tracks `/api/tracks/tracks`, routing
  `/api/route/*`). This doc only defines the **rendering** path for that geometry.
- **State:** Zustand slices — `location` (last fix, permission status),
  `camera` (current/persisted). Entity sources are assembled by the compositor from
  feature-owned TanStack Query data.

## Renderer integration
This doc defines the **shared overlay compositor** all later feature docs build on.

- **Compositor (React module, e.g. `useSceneCompositor`):** a single place that
  takes the current inputs —
  `{ baseLayer, terrainOn, water/hillshade/sun flags, activeMapOverlays (doc 04),
    entityOverlays: geo-json contributions from features, tubeRequests }` —
  and produces **one** `Scene` (sources + ordered layers), then calls
  `apply_scene(JSON.stringify(scene))` whenever inputs change. This guarantees
  toggling one thing never rebuilds unrelated state and keeps draw order
  deterministic (base → terrain shading → map overlays → entity lines/fills →
  symbols/circles → pins(DOM)).
- **Entity layers in the Scene:**
  - tracks / route lines → `geo-json` source + `line` layer
  - checkpoints / measure dots / (optional) my-position → `geo-json` + `circle`
  - markers / waypoint / origin pins → `geo-json` + `symbol`, **or** DOM overlays
    for richer interactivity (the default for pins is DOM, matching Android's
    Compose overlays; `symbol` is the lighter alternative).
- **Raised tubes:** `set_route_tube(id, points, color, radiusPx)` — **passthrough to
  add to `turbomap-web`** (engine + core already implement it). Used for route ahead
  (bright), walked segment (dim), and track tubes.
- **Projection for DOM overlays:** `map.project(lat,lng)` per visible entity; the
  heading beam angle from projecting fix + a point ~20m ahead along `heading`.
- **`turbomap-web` methods called:** `apply_scene`, `set_camera`/`ease_to`,
  `camera_json`, `project`/`unproject`, `set_viewport_inset`, and (after
  passthrough) `set_route_tube`.
- **Scene IR change:** widen `Layer` union (`line`, `fill`, `symbol`, `circle`) in
  `apps/web/src/map/scene.ts`.

## Out of scope (this phase)
- Background / locked-screen GPS, foreground service, lock-screen widget
  (Android-only).
- Continuous follow-camera (doc 11) — this doc covers one-shot recenter + first-fix
  center only.
- Recording/route geometry *production* (docs 09/10/11) — only their rendering path
  is defined here.
- Server-side camera sync as a hard requirement (localStorage is the baseline).

## Open questions
- **Pin rendering choice:** DOM/Canvas overlays (rich, matches Android Compose) vs
  `symbol`/`circle` Scene layers (terrain-draped, lighter)? Proposed default: DOM
  for interactive pins, Scene layers for dense/non-interactive sets — confirm.
- **Server camera mirror:** persist camera under account settings (doc 19) in
  addition to `localStorage`, or `localStorage` only for now?
- **`set_route_tube` radius units:** standardize on **pixels** (`radius_px`, as core
  expects) for the web passthrough — confirm and align the Android JNI naming
  (`radius_m`) to avoid the existing mismatch.
- **Accuracy ring:** include the GPS-accuracy ring around the dot for web parity, or
  defer as polish?
