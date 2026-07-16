# TURBO map overhaul — product spec, test targets, TDD prep

**Date:** 2026-07-16. **Status:** agreed via design interview; ready for TDD.
**Platforms:** Android native + web. **Web-mobile mirrors Android one-for-one** — the
current web-mobile UX is a first-class target, not an afterthought (it looks nice but plays
sub-par today). iOS is out of scope for this document.

This spec turns the TURBO product notes into buildable, testable behavior. Every section states
the **corrected behavior**, the **seam** that makes it drivable without a real touch, and the
**high-value test targets** — tests that assert a unit of *user-visible behavior*, nothing else.

---

## 0. Testing philosophy (governs everything below)

The recurring wall in this codebase is *"adb/Compose can't drive gestures; the harness can't
repro."* Half of this spec is gestures and drags. So the rule for the whole overhaul:

> **Every user-facing behavior has a pure, drivable seam below the raw-touch / render layer, and
> the test exercises that seam — never a scripted real touch.**

Three tiers, in value order:

1. **Tier 1 — pure gesture/logic units.** Tap/long-press guards, the rotation gatekeeper, the
   fling settle integrator, the avoidance penalty math. Extracted as pure functions/reducers over
   `(pointerId, position, timeMs)` event streams → decisions (`TapAt`, `Rotate`, `PanZoom`,
   `Cancelled`, …). Tests feed hand-authored event sequences and assert the decision. No Compose,
   no device.
2. **Tier 2 — ViewModel / widget-state.** Rail collision layout, compass auto-hide, quick-actions
   state machine, stops reorder, sun/3D decoupling at the scene-IR level, weather-pin
   persistence. Pure JVM / Robolectric / vitest.
3. **Tier 3 — engine.** Router avoidance + round-trip self-avoidance, flick integrator: Rust unit
   tests on synthetic fields; `turbomap-app/examples/scenario.rs` for anything needing a real
   render.

**Deliberately not automated:** that a proven pure unit is *wired* to Compose's real pointer
input and that pixels land right. That residue is a **short manual device-QA checklist** (§8) —
one line per gesture — kept small because all *judgment* lives in tiers 1–3.

**Hard consequence (Phase 0):** gesture decision logic is refactored into pure units *before* any
new gesture behavior is added. Without it, rotation/tap/drag stay permanently device-only.

**High-value bar:** a test earns its place only if it asserts behavior a user would notice
breaking. No interaction/mock-verification tests, no call-count assertions, no "the reducer has a
field" tests. The unit under test is a *unit of behavior*, not a class's method.

---

## Phase 0 — Gesture seam extraction (no user-visible change)

Foundation. Ships nothing; unblocks everything.

**Work:** extract from `MapGestureDetector` into pure units:
- `TapGuard` — decides `TapAt(pos)` vs `Ignored` from a down→up sequence + movement.
- `LongPressGuard` — decides `Fire` after `LONG_PRESS_MS`, `Cancelled` on movement past the
  guard, and enters `Suppressed` until touch-up→touch-down.
- `RotationGatekeeper` — per two-finger gesture, decides `Rotate` vs `PanZoom` and holds the
  verdict for the gesture (the sequence lock).
- `FlingIntegrator` — velocity → per-frame camera decay to settle.

**Tunables** become a named `GestureConfig` (defaults below), sourced from a new **Settings →
Gestures** section so users can tune feel:
| Tunable | Default | Meaning |
|---|---|---|
| `longPressMs` | 500 | long-press fire delay |
| `movementGuard` | platform `touchSlop` (~8dp) | tap-ignore / long-press-cancel radius |
| `rotationGateDeg` | 10° | twist that must accrue before pinch/pan to engage rotation |
| `flingHalfLifeMs` | 300 | fling velocity half-life (higher = floatier) |

**Test targets (Tier 1):**
- Tap with total movement ≤ `movementGuard` ⇒ `TapAt`; movement > guard ⇒ `Ignored` (boundary
  case at exactly slop).
- Long-press held `LONG_PRESS_MS` without movement ⇒ `Fire`; movement past guard before the timer
  ⇒ `Cancelled`, and subsequent moves in the same touch sequence stay `Suppressed` until a fresh
  down.
- `GestureConfig` override flows into each guard (set `longPressMs=400` ⇒ fires at 400, not 500) —
  proves the settings section actually drives behavior, not just persists.
- Fling: given initial velocity v, camera settles (<0.5 px/frame) within a bounded frame count and
  bounded total travel; re-tuning `flingHalfLifeMs` moves the assertion, not device feel.

---

## Phase 1 — Button rail + chrome overhaul

### The five-button rail
Exactly five, top→bottom: **Compass (auto-hide)**, **Layers**, **Location**, **+**, **−**. The
four evicted controls relocate: Add-Marker + Route → quick-actions card (Phase 2); 3D + Sun →
layers-sheet sliders (below).

### Adaptive collision ("smart slide")
Side widgets slide vertically to fit but **never overlap the search bar**; **+/−** auto-hide when
contextual widgets scale up into their vertical space.

- **Seam:** a pure `RailLayout(searchBarBounds, widgetSizes, screenHeight) → List<Offset> + visibleFlags`.
- **Test targets (Tier 2):** computed positions never intersect the search-bar bounds at any
  screen height; when a contextual widget claims the +/− band, their `visible` flags go false;
  shrinking it restores them. (No pixels — geometry.)

### Compass widget
- **Auto-hide:** hidden (fade+slide) when bearing within ~0.5° of north; visible otherwise; stays
  live in 3D (orbit changes bearing).
- **Tap** = animate to north (`resetNorth`). Icon shows heading with a radial FOV wedge
  (descriptive; no behavioral test — cosmetic).
- **Long-press → mini-menu with "Lock rotation"**: persisted; suppresses gesture bearing changes
  in *both* modes (2D twist + the horizontal component of 3D orbit); pitch stays free. Locked +
  north ⇒ compass stays hidden.
- **Test targets (Tier 2):** bearing 0.3° ⇒ hidden flag; 15° ⇒ visible; lock on ⇒
  `RotationGatekeeper` yields `PanZoom` even for twist-first input (config-driven), pitch orbit
  still allowed.

### Layers sheet (the new control center)
- **Basemaps:** Topo / OSM / Satellite + user custom sources. Web-mobile mirrors exactly
  (drop web's 4th "Topo world" entry on mobile for parity).
- **Trails + Clouds:** removed from the layers sheet, gated behind **Settings → Experimental**
  toggles (off by default; the layer row only appears when its experiment is on). Avalanche stays
  (safety overlay, unnamed by the spec).
- **3D Model Slider:** 0 = flat 2D; >0 = 3D — *unlocks tilt/orbit gestures* and scales terrain
  exaggeration 0→8 (default detent 6). Dragging to 0 returns to 2D and re-locks tilt.
- **Sun Position Slider:** available in **both** modes. Sun > 0 in 2D activates the DEM + relief
  lighting **camera-locked top-down** (sun-lit relief seen from above); the 3D slider is what
  unlocks tilt. **Hard decoupling rule:** *neither slider ever changes camera pitch/position.*
  This kills today's "enabling sun forces a 3D tilt."
- **Test targets (Tier 2, scene-IR level):**
  - 3D slider 0 ⇒ scene has no DEM + pitch gestures rejected; >0 ⇒ DEM present with exaggeration
    value + pitch gestures accepted.
  - Sun >0 in 2D ⇒ scene has DEM + sun vector **and** camera pitch == 0 **and** pitch gestures
    still rejected (the top-down-lit case).
  - Moving the sun slider ⇒ environment sun vector changes while the camera pose object is
    byte-identical (the decoupling test).
  - Experimental toggle off ⇒ Trails/Clouds rows absent from the layers sheet; on ⇒ present.

---

## Phase 2 — Quick-actions card + tap semantics

### Tap semantics (replaces the long-press-only menu)
- **Tap on empty map** ⇒ opens the unified **map-point card**, anchored at the point.
- **Tap on an entity** (marker/pin/POI) ⇒ opens that entity's detail (entity hit wins).
- **Long-press anywhere** ⇒ opens the card even over an entity (drop-on-top).
- **Card open + tap elsewhere** ⇒ re-anchors to the new point (resets expansion), not dismiss.
  Dismiss = pan/flick, close affordance, or back.
- **Track/route mode** ⇒ card never opens; taps place/extend points (movement guard applies).
- **Seam:** a `MapPointCardState` reducer over `(tap|longPress|entityHit|panStart|trackModeActive)`.
- **Test targets (Tier 2):** empty tap ⇒ Open(point); entity tap ⇒ EntityDetail, card stays
  closed; second tap ⇒ re-anchored + collapsed; pan ⇒ Closed; trackMode ⇒ tap yields PlacePoint,
  never Open.

### The card's actions
Header = geocoded place name + current temp (`ConditionsRepository` + reverse geocoder; "—" when
offline). Row adapts:
- **Add Marker (collapsed ∨ → expanded ∧)** = expansion only, revealing **Standard Marker /
  Photo / Weather Pin**. Standard opens the create-marker sheet pre-filled with the geocoded name.
- **Route Here** = `planRoute(from = userFix ?: cameraCenter, to = point)`, opens track tool in
  Route mode.
- **Measure** = **online:** opens the track tool (Line mode, tapped point pre-placed as vertex 1).
  **Offline:** disabled with an explicit "not available offline" message. Gated on `NetworkMonitor`.
  (Product choice — Line geometry is local; margin note: could work offline later.)
- **Photo (expanded)** = camera/gallery picker → creates a **marker at the point with the photo
  attached** (not a new entity).
- **Test targets (Tier 2):** Route Here ⇒ `planRoute` called with (userFix→point), Line-mode not
  entered; Measure online ⇒ track mode = Line, vertex1 = point; Measure offline (fake connectivity)
  ⇒ action disabled + message, track tool NOT opened; Photo ⇒ marker created with photo ref;
  header shows fake name+temp, "—" when the fakes fail; Add-Marker tap ⇒ expanded state only, no
  new screen.

---

## Phase 3 — Weather pins

A live, persistent, cached map node — modeled as a **`Marker` kind (`WeatherPin`)**, reusing
marker persistence, sync (DB migration adds the kind), and on-map rendering. Not a new entity.

- **Live + cached:** on drop and each open, fetch `ConditionsRepository` (MET); cache last
  forecast + fetch timestamp on the node. Renders instantly from cache (offline-safe, shows
  "updated Nh ago"); refreshes when stale (>1h) and online.
- **Standard view:** temp + condition glyph. **Expanded:** wind speed/direction, precipitation,
  wave/marine data (existing ocean tab fields).
- **Test targets (Tier 2):** node round-trips as a marker kind through the repository; renders
  cached forecast with a fake conditions repo returning stale cache + no network (no exception);
  refreshes when stale+online, keeps cache when offline; expanded view surfaces wind/precip/marine
  fields.

---

## Phase 4 — Track Gen fixes

Polish on code that shipped recently.

- **Rail dead in track mode = z-order/occlusion, not logic.** The track sheet renders over the
  rail. Fix: rail sits **above** the sheet in z-order and slides up to clear the sheet's inset
  (the §1 smart-slide). **Test (Tier 2):** with the sheet at each detent, rail buttons are
  un-occluded and their callbacks fire (hit-test/semantics, not pixels).
- **Dead slider = the sheet's drag handle** (the grabber), not an elevation slider. It should drag
  the sheet between detents but doesn't respond. Fix: wire the handle to detent state
  (anchoredDraggable/nestedScroll). **Test (Tier 2):** a drag on the handle moves the sheet's
  detent state through its stops — asserted at the gesture-callback/state seam (Compose drag can't
  be adb-scripted; that's why the detent logic is a drivable unit).
- **Route Styles → remove the UI.** Delete the preset/style selector from the route card; keep the
  enum/solver plumbing (default preset). **Test (Tier 2):** route card exposes no style selector;
  solving still returns a route on the default preset.
- **Stops widget:**
  - **Color retention:** stops keep their categorical palette color in the list and on the map
    across re-solve and reorder. **Test:** color survives a `moveWaypoint` + re-solve.
  - **Geocoded names (lazy + cached + no layout shift):** each stop reverse-geocodes lazily via
    `ReverseGeocodeRepository`, caches the name on the waypoint (~11 m grid), and **never blocks
    the solve**. Display rule: cached name → else fetch-if-online → else **trimmed coords**
    (`69.9607, 23.2715`). The row renders coords immediately in the *same single-line, fixed-height
    slot* the name will occupy, so resolving is an in-place text swap — **no reflow**. **Tests
    (Tier 2):** fake geocoder returns name ⇒ shows name; offline ⇒ trimmed coords, no exception;
    re-render ⇒ no second geocode (cache hit); reorder preserves name+color; row is
    single-line/fixed-height so the coords→name transition can't shift layout.
  - **Draggable reorder → re-solve:** drag handle reorders stops; drop fires
    `moveWaypoint(from,to)` and a re-solve. **Test:** reorder ⇒ waypoint order changes + solve
    fired once.
- **Round Trip (client):** toggle appends the origin as the final destination and re-solves.
  **Test:** on ⇒ solved geometry starts and ends at origin; off ⇒ reverts.

---

## Phase 5 — Avoid Marked + round-trip self-avoidance (backend)

Highest risk, most isolated. The router (`turbo-tiles-pathfind`) already exposes the seam:
`Prefs.layer_weights` (per-layer cost multipliers) and cell cost composition.

### The core model: **edge-based penalty, not a spatial corridor**
A naive spatial buffer (penalize cells within 30 m of avoided geometry) fails badly: the router
escapes by bushwhacking ~31 m *parallel* to the trail, off-trail, forever — "shadow-walking." So:

- Avoidance penalizes the **graph edges (trail segments)** the avoided geometry uses, as an
  edge-cost multiplier in the **graph Dijkstra leg** — *not* a spatial off-trail corridor.
- Off-trail base cost stays high, so parallel-off-trail is never cheaper than a real alternative
  trail. The return leg's cheapest option becomes a **divergent marked trail**; only when none
  exists does re-using the outbound segments (on-trail backtrack) win. It never shadow-walks.
- The **configurable radius** is the *edge-projection distance* for avoided geometry that isn't
  itself on the graph (freehand routes) — how far to project the penalty onto nearby edges — not a
  no-go tube width. Default ~30 m, a request field + Settings exposure.
- **Soft penalty (default):** strong multiplier, not infinite — a start point on an existing trail
  still yields a route (it peels off ASAP) rather than "no route."

### API
`RoutePlanReq` / stream request gain:
- `avoid: [[[lon,lat],…], …]` — polylines to avoid (rasterized/projected to a transient penalty
  layer per request; no persistence, no graph rebuild).
- `avoid_radius_m: f64?` — edge-projection distance (default from config).
- `round_trip: bool` — when true, server solves two legs: (1) origin→vias→far point; (2) far
  point→origin with the **outbound leg's edges injected into the avoid layer** so the return
  diverges. Soft, so a single-path spur gracefully returns an out-and-back.

### Client
- Route card "Round Trip" toggle (Phase 4) + "Avoid Marked" toggle. Avoid Marked collects the
  geometries of *other* routes/tracks currently on the map and passes them as `avoid`. Toggling
  re-solves.

### Test targets (the behavioral wins, Tier 3 Rust on synthetic graphs)
- **Detour when possible:** endpoints with an `avoid` corridor straddling the straight path ⇒
  returned geometry's overlap with avoided edges below threshold (it detours).
- **Route-through when forced:** same, but no alternative ⇒ still returns a route (soft penalty),
  not NoRoute.
- **No shadow-walking (the key test):** graph = trail A→B + a parallel off-trail gap + a separate
  *divergent* trail. Round-trip-avoid return leg must land on the **divergent trail**, with
  off-trail metreage ~0. Fails if the router walks parallel off-trail instead of taking the real
  alternative. This single assertion encodes the whole product objection.
- **Graceful loop:** two parallel trails A↔B ⇒ round-trip-avoid returns a loop whose return leg
  overlaps the outbound below threshold (uses the second trail); single trail ⇒ returns the
  out-and-back rather than failing.
- **Client (Tier 2):** Avoid Marked on ⇒ solve request carries the other on-map geometries in
  `avoid`; off ⇒ field absent. Round Trip on ⇒ `round_trip:true` sent.

---

## Web-mobile mirror

Each phase's web counterpart lands **with** its Android phase (not batched at the end, so the two
can't drift). Web-mobile must reach behavioral parity with Android: five-button rail + collision,
tap-opens card, layers-sheet sliders with the same decoupling rule, weather pins, stops widget,
Round Trip / Avoid Marked. Engine-level pieces (gesture math via the shared Rust/gesture layer,
router avoidance) are shared, so web inherits them; the React chrome is the per-phase work. Parity
itself is a test target: the same Tier-2 behavioral assertions run against the web ViewModels/stores.

---

## Manual device-QA checklist (the residue tiers 1–3 can't cover: wiring + feel)

One line each, run once per gesture-touching phase on the emulator:
- [ ] Tap empty map opens the card at the point; tap a marker opens its detail instead.
- [ ] Two-finger twist-first rotates; pinch-first does not rotate for that gesture.
- [ ] Long-press fires the card at ~500 ms; a drag during it cancels cleanly.
- [ ] Fling settles smoothly (no float/overshoot) at the tuned damping.
- [ ] Rail buttons work with the track sheet open (not occluded).
- [ ] Sheet drag handle moves the sheet between detents.
- [ ] Compass hides at north, reappears when rotated, tap returns to north.
- [ ] Sun slider in 2D lights the relief top-down without tilting the camera.

---

## Deferred / descriptive-only (no behavioral test)
- Compass radial FOV wedge rendering (cosmetic).
- Menu entry animation "smooth/fluid" (animation, not behavior — the state transition is tested,
  the easing is not).
- Measure working offline (product chose online-gated; revisitable — Line geometry is local).
