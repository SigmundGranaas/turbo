# Tracking / Following Redesign — Implementation Plan (Android + iOS)

Companion to the behavioural spec `2026-06-tracking-following-redesign.md` (8 user
stories; decisions D1/D4/D6/D7 locked). This plan is the *how*: shared contract,
domain model, module placement, phased work per platform, and test strategy.

## Cross-platform strategy: one contract, two native implementations

The apps share no runtime code (Kotlin vs Swift; the only shared binaries are the
Rust routing/tiles crates, irrelevant here). "Lockstep" therefore = a **shared
contract**, not shared code:

1. **One algorithm spec** per pure component (RouteProgress cursor, LocationFilter,
   ETA), written once, ported to both languages verbatim in behaviour.
2. **Golden fixtures** — language-agnostic JSON of `input → expected output` —
   checked in once at `fixtures/tracking/` (repo root). Both platforms' unit tests
   load the *same files* and assert. A divergence becomes a failing test, not a
   drift nobody notices (this is precisely what bit us with pause semantics).
   - iOS: fixtures copied into the test target's resources via SwiftPM
     `resources: [.copy(...)]` (a tiny prebuild sync script keeps them current).
   - Android: read directly as test resources / relative path from the module.
3. **Behaviour parity checklist** for the non-pure, UI-coupled parts (camera,
   drag, sheet) that fixtures can't cover — each gets a manual device-QA line item
   plus whatever automated coverage is reachable.

Pure components are the bulk of the risk (progress, filtering) and are fully
fixture-covered. UI components are platform-idiomatic and verified per platform.

## Shared domain model (mirrored on both platforms)

| Type | Responsibility | iOS home | Android home |
|------|----------------|----------|--------------|
| `FilteredFix` | An accepted location reading (pos, accuracy, speed, alt, heading, timestamp) | CoreData | core/data |
| `LocationFilter` | Stateful gate: accuracy + staleness + jump; holds last-accepted fix | CoreData | core/data |
| `RouteProgressTracker` | Stateful arc-length **cursor** over a planned route; per-fix produces `RouteProgress` (fraction, distanceRemaining, eta, offRoute, arrived) + phase crossings | CoreModel | core/model |
| `RouteProgress` | Immutable per-fix snapshot | CoreModel | core/model |
| `Phase`/`Checkpoint` | A route sub-point + its crossing (timestamp, split time/distance) | CoreModel | core/model |
| `TrackSession` | THE capture engine: travelled track + stats + pause/buffer + draft; consumes `LocationFilter` stream; app-lifetime | **new CoreTracking** | core/data |
| `RouteGuidance` | Optional component on a session in Follow mode: planned route + `RouteProgressTracker` + phases + dim-split | **new CoreTracking** | core/data |

**Why a `TrackSession` + `RouteGuidance` split (not two controllers):** Record and
Follow are one activity. `TrackSession` always records; `RouteGuidance` is "is
there a route attached?". This deletes the `RecordingController`/`FollowController`
duplication and guarantees identical stats (US-1) by construction.

**iOS module note:** new `CoreTracking` SPM target depends on CoreData
(LocationProvider/filter, PathRepository) + CoreModel (RouteProgress). `AppContainer`
owns the single `TrackSession`; `FeatureRecording` (RecordingScreen) and
`FeatureMap` (FollowCard, map pill) both add the dep and *observe* it. Collapses
`RecordingController` (FeatureRecording) + `FollowController` (FeatureMap).

**Android module note:** unify inside `core/data` (where both controllers,
`LocationRepository`, and `RecordingDraftStore` already live); `RecordingService`
(foreground) drives the one `TrackSession`. Progress lives in `core/model`.

**Renderer coordination (Android):** camera-follow-release (US-6) and waypoint
move (US-7) touch the map layer while the MapLibre→wgpu renderer swap is in flight
([[android-renderer-swap]]). Implement against the **MapEngine seam** so behaviour
holds for both renderers; if the seam doesn't yet expose gesture callbacks, that's
a small seam addition, flagged in Phase 3/6.

---

## Phase 0 — Fixtures & contract harness

**Goal:** the shared test contract exists before any algorithm is written
(test-first across platforms).

- Create `fixtures/tracking/` with the JSON schema documented in a README:
  - `progress/`: `straight.json`, `out-and-back.json`, `loop.json`,
    `skip-ahead.json`, `off-route-and-return.json`, `backtrack.json` — each
    `{ route:[[lat,lng,ele?]…], fixes:[[lat,lng]…], expect:[{fraction,
    remainingM, offRoute, arrived, phaseCrossed?}…] }`.
  - `filter/`: `resume-stale.json`, `low-accuracy.json`, `teleport-jump.json`,
    `valid-walk.json` — `{ fixes:[{lat,lng,accuracyM,ageMs,speedMps?}…],
    acceptedIndices:[…] }`.
- iOS: SwiftPM resource copy + a `FixtureLoader` test helper. Android: a
  `fixtureJson(name)` test helper.
- **DoD:** both platforms can load a fixture and a trivial round-trip test passes.

## Phase 1 — RouteProgress cursor (US-2 + US-3 data)

**Goal:** kill the loop/out-and-back bug; produce phase splits. Highest value,
purely testable.

**Shared spec (the algorithm):**
- Precompute cumulative arc-length per route vertex.
- `RouteProgressTracker` holds `sCursor` (start 0) + per-phase crossed flags.
- Per accepted fix: nearest route point **within window** `[sCursor−BACK,
  sCursor+AHEAD]` (BACK 60 m, AHEAD 400 m); `sCursor = max(sCursor, sMatch)`;
  `fraction = sCursor/total`; `remaining = total−sCursor`; `eta` from remaining +
  remaining ascent (Naismith, unchanged). `offRoute` = perp dist > 50 m for N=3
  fixes. `arrived` = `sCursor ≥ total−tol` **AND** within 30 m of the actual end.
- Phase crossings: when `sCursor` passes a phase's arc-length, emit a crossing
  with timestamp + split since previous phase.

**iOS:** new `Sources/CoreModel/RouteProgress.swift` (`RouteProgressTracker`,
`RouteProgress`, `Phase`). Keep `GeoMetrics.haversine/pathLength/projectFraction`;
**delete `GeoMetrics.progress`** (global-nearest) and its callers' use.
**Android:** `core/model` `RouteProgress.kt`; replace `GeoMetrics.progress`.

**Tests:** both run all `progress/*` fixtures. Add the explicit assertions the
current code lacks: out-and-back is ~50 % at turnaround, ~100 % at return, **never
`arrived` before the turnaround**, remaining strictly decreases on the return leg.

**DoD:** all fixtures green on both platforms; old `progress` removed; no caller
references it.

## Phase 2 — LocationFilter + resume guard (US-5)

**Goal:** no teleport-then-snap-back; clean stream for every consumer.

**Shared spec:** stateful filter holding the last accepted fix. Reject if:
horizontal accuracy > 50 m; age > 5 s; or implied speed vs last accepted >
30 m/s unless K consecutive consistent fixes confirm (so a genuine fast move
eventually passes). On resume, suppress camera/dot move until the first fix passes
all gates.

**iOS:** `Sources/CoreData/LocationFilter.swift`; apply inside `CoreLocationProvider`
(filter before `emit()`), so `fixes()` is already clean for map dot, session, and
follow. Need `horizontalAccuracy` + `timestamp` → extend the CL bridging.
**Android:** `core/data` `LocationFilter`; apply in `LocationRepository` before it
emits. Replace `RecordingFilter`'s lone accuracy gate (now redundant).

**Tests:** both run `filter/*` fixtures. Plus an injected "stale fix on resume"
case asserting the dot/recorded distance don't move to it.

**DoD:** fixtures green; map dot + recording + follow all consume the filtered
stream; manual: backgrounding then resuming shows a stable position.

## Phase 3 — Camera follow-release on manual pan (US-6)

**Goal:** auto-follow on open + during record/follow; manual pan releases instantly.

**Behaviour:** follow state defaults ON at app open and while a session is active;
a *user-initiated* gesture (pan/zoom/rotate) sets it OFF; the follow control
re-enables. Camera updates use the filtered stream, lightly smoothed.

**iOS:** `TurboMapView` — detect user gestures (pan/pinch/rotation recognizers, or
`mapView(_:regionWillChangeAnimated:)` gated by a "user interaction" flag set from
the gesture recognizers' state) and call back `onUserPannedMap`; `MapViewModel`
sets `following = false`. Set `following = true` in `MapScreen.task` (currently it
only calls `enableLocation()`). Distinguish programmatic recenters (don't
self-cancel).
**Android:** map screen — add a gesture listener (via MapEngine seam) → drop
follow; set follow on at open + session start.

**Tests:** iOS E2E reachable (rail follow button toggles; a programmatic region
change must NOT disable follow — assert state). Manual device QA: open → follows;
pan → stops; tap → resumes; no fighting during record.

**DoD:** behaviour parity checklist passes on both; no self-cancel on programmatic
moves.

## Phase 4 — Session unification: Follow = Record + auto-save (US-1, US-3 visual)

**Goal:** one engine; following records identical stats and **auto-saves** (D1);
covered guide dims; phases render.

**iOS:**
- New `CoreTracking`: `TrackSession` (absorbs RecordingController's capture +
  stats + start/pause/resume/stop) consuming the filtered stream; `RouteGuidance`
  (holds `FollowRoute` + `RouteProgressTracker` from Phase 1).
- `AppContainer` owns one `TrackSession`; delete `RecordingController` +
  `FollowController`; repoint `RecordingScreen`, `FollowCard`, map pill,
  `RootView` wiring.
- Follow start = start a session with guidance attached. Finish = **auto-save**
  the travelled track (guard: skip < ~50 m / trivial), keeping planned-route +
  phase-split refs on the `SavedPath`. Plain recording keeps its Stop→save/discard
  prompt (intentional asymmetry).
- Map draws three things: planned-remaining (highlight), planned-covered (dim,
  split at cursor), travelled track (distinct). `TurboMapView` route rendering
  extended to multi-polyline styles.
- `SavedPath`/`GeoPath` gains optional `plannedRouteRef` + `phaseSplits`.

**Android:** same unification in `core/data` (`TrackSession` + `RouteGuidance`);
`RecordingService` drives the session; `LiveSheet` reads one read-model for both
modes (already close — `LiveStats`); add the dim-split + phase rendering; auto-save
on follow finish. **Plus, fix the existing LiveSheet progress UI** (current build,
misbuilt):
- Collapse to **ONE** progress control — remove the duplicate slider/bar.
- Drive a single Material 3 `LinearWavyProgressIndicator` from
  `RouteProgress.fraction`: **wavy = completed/tracked** portion, flat ahead.
- **Slow the wave right down** (it's "crazy fast" today) — a calm, near-static
  undulation; cut the animation speed/spec so it reads as progress, not motion.
- **Always show accumulated distance + ascent (gain) + descent (loss)** + moving
  time as the primary stats in BOTH record and follow modes — never behind an
  expand (US-1 always-visible stats). Confirm the iOS recording grid already does
  this (it does); bring Android to the same always-on readout.

**Tests:** unit — a follow over a fixture route yields the same distance/time/
ascent as a recording of the same fixes (parity assertion); auto-save guard. E2E
(iOS, via rail entry + saved-track follow): follow a saved track → finish → it
appears in Paths automatically. Phase-split unit tests from Phase 1 data.

**DoD:** one engine; follow auto-saves a real track equal to a recording; dim guide
+ phases visible; old controllers deleted.

## Phase 5 — Pause buffer + auto-resume nudge + draft (US-4)

**Goal:** pause keeps capturing into a held buffer; nudge on movement; Include/
Discard on resume; backgrounded sessions survive process death.

**Behaviour (D4):** pause → fixes go to a `heldBuffer`, moving time + distance
freeze. If buffer movement > 80 m or sustained motion > 60 s → proactive nudge
(notification + in-app banner). Resume → if buffer is meaningful, prompt "Include
the N m moved while paused?" → Include stitches (back-dated) / Discard drops.

**iOS:** add `pause()`/buffer to `TrackSession` (today only start/stop/resume,
fixes dropped while !recording); a `DraftStore` (mirror Android's
`RecordingDraftStore`) persisting the in-progress track + buffer so a backgrounded
session recovers; wire the nudge to the existing Live Activity / a local
notification.
**Android:** `RecordingController` already buffers while paused + has
`RecordingDraftStore` — add the movement nudge + the Include/Discard-on-resume
prompt (currently buffered points are just held); surface via the foreground
notification.

**Tests:** fixture — pause, feed moving fixes, resume → Include yields continuous
track, Discard yields a gap. Draft round-trip (save draft → reload → identical).

**DoD:** forgotten-unpause never silently loses or pads distance; recoverable after
kill on both platforms.

## Phase 6 — Reliable waypoint move + whole-sheet drag (US-7, US-8)

**Goal:** every track point moves reliably via **both** drag and tap-to-place
(D7); the whole live sheet is grabbable.

**Waypoint move:**
- **iOS:** tap-to-place already landed and is E2E-tested — keep as the verified
  path. Make **drag** bulletproof: the `draggingPinIds` snap-back guard must hold
  across re-solves and rapid sequential edits; on-device QA moving the 1st, 3rd,
  5th points in a row with route-mode re-solving between each. Investigate the
  reported "first works then snaps back" — likely the route re-solve rebuilding
  annotations mid-interaction; ensure the guard + graceful re-solve cover it.
- **Android:** `moveWaypointTo` exists; verify on-device drag holds across the
  300 ms re-solve; add tap-to-place parity + the move-mode banner; mirror the
  snap-back guard if the renderer rebuilds annotations.
- Shared: an automated map-tap test per platform (reachable via the rail track
  entry) moving several points in sequence.

**Whole-sheet drag:**
- **iOS:** `RecordingScreen`/live sheet — make the entire surface drive the detent
  drag (not just a handle); coexist with inner scroll (drag sheet at scroll-top).
- **Android:** `LiveSheet` uses `AnchoredDraggable` on a handle/regions — extend
  the drag source to the whole sheet; fix swipe-down-to-collapse settling.

**DoD:** sequential multi-point moves hold on both platforms (device QA + the
automated tap test); sheet drags from stats/title/body.

---

## Sequencing & dependencies

| Phase | Depends on | Parallelizable | Notes |
|-------|-----------|----------------|-------|
| 0 Fixtures | — | — | Do first; unblocks 1, 2 |
| 1 Progress cursor | 0 | ✓ (pure) | Highest value; unblocks 4 |
| 2 LocationFilter | 0 | ✓ (pure) | Independent feel-win |
| 3 Camera release | — | ✓ | Independent feel-win; small |
| 4 Session unify | 1 (2 ideal) | — | Architectural core |
| 5 Pause/buffer/draft | 4 | — | Builds on session |
| 6 Waypoint + sheet | — | ✓ | UI polish; independent |

**Working order (keeps platforms in lockstep, fast feedback):** per phase →
(a) shared fixtures/spec, (b) **iOS** (deep context + sim test loop), (c)
**Android** against the same fixtures, (d) device-QA the UI-coupled bits. Land
phases 1→2→3 (quick, high-feel, independent) before the bigger 4→5, with 6
slotted whenever. Each phase is independently shippable.

## Test strategy

- **Pure logic (1,2, parts of 4/5):** golden fixtures, both platforms, identical
  expectations — the lockstep backbone.
- **Reachable E2E:** iOS XCUITest via the **rail entries** (track button, follow
  from saved track) + **map-coordinate taps** (long-press is not E2E-drivable —
  established constraint). Covers: follow→auto-save→appears in Paths; tap-to-move
  a point; follow card lifecycle.
- **Device-QA checklist (not automatable):** camera follow/release feel; pin
  **drag** across sequential edits; teleport-on-resume stability; whole-sheet drag;
  pause-while-moving nudge. Each phase lists its manual lines; run on a real device
  before calling a phase done (this is the QA gap that bit waypoint-move — UI map
  gestures must be hand-verified, not assumed from unit/type-check green).

## Risks

- **Out-and-back cursor edge cases** (genuine long backtrack, big shortcut): tuned
  by window sizes (D2); fixtures `backtrack.json`/`skip-ahead.json` pin behaviour.
- **iOS draft persistence** is net-new (Android has it) — keep it small (Codable
  snapshot of session+buffer to disk on each append/scenephase).
- **Android renderer swap in flight** — do camera/waypoint work against the
  MapEngine seam; may need a small seam addition for gesture callbacks.
- **Session unification is invasive** (deletes two controllers, repoints all
  wiring) — gate behind full unit + E2E + device QA before deleting the old paths.
