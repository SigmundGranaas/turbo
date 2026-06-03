# UX Integration & Composition Audit — Flutter App

**Date:** 2026-06-02
**Scope:** `apps/flutter/lib` (461 Dart files, 29 feature folders)
**Trigger:** After shipping the route-building feature, the app *feels* disjointed
— duplicated features that don't talk to each other, new screens where there
should be in-place UI, uncoordinated overlays, and "static" widgets that should
be live.

> **Guiding principle for the fix:** *Composition over inheritance.* We compose
> small features into the map, we do **not** grow god modules or deep class
> hierarchies. Every recommendation below is a **seam** — a place where a feature
> plugs in — not a base class to inherit from.

---

## 1. Executive summary

The symptom the user describes ("feels like features were built by different
people") is real and has **one root cause**:

> **There is no composition layer. `MainMapPage` is a god-widget, and every
> "tool" (route, measure, record, navigate) is either hand-wired into it or
> escapes into its own full-screen page with its own map. Features own
> disconnected state silos with no shared "what is the user doing right now"
> concept, so nothing composes.**

Concretely, the codebase has **four** structural defects, each producing a
cluster of the "feels off" symptoms:

| # | Defect | User-visible symptom |
|---|--------|----------------------|
| A | **Screen proliferation / map duplication** — 5 separate `MapController`/`FlutterMap` instances; tools push full-screen pages | "Drawing/creating paths opens new screens instead of replacing the UI." |
| B | **No overlay coordinator** — flat hand-built `Positioned` list + 58 imperative `showModalBottomSheet`/`showDialog` calls, zero mutual exclusion | "Popups and overlays are static, we end up with multiple widgets overlaying." |
| C | **Live-state silos** — follow / navigate / record / route are 4 unaware state machines; no "active journey" concept | "Tracking just opens a widget instead of integrating; generated route is static, my position isn't live." |
| D | **Concept duplication, no seams** — 5 representations of "a polyline path", 6 copy-pasted activity features, layers hard-wired into the god-widget | "Tons of duplicated, related features that don't complement each other." |

Notably, the project's **own** architecture doc
(`lib/context/architecture.context.md`) already mandates feature encapsulation
and "prevent features from becoming tightly coupled." The implementation
violates this: `MainMapPage` imports ~20 features directly and is the *only*
integration point. The doc got the `api.dart` boundary right (it's genuinely
respected — zero cross-feature reach-ins found) but never defined the
**composition seams** that would let features integrate *with each other*
instead of all funnelling through one god-widget.

---

## 2. Defect A — Screen proliferation & map duplication

### Evidence

**Five distinct interactive map instances**, each with its own `MapController`,
its own tile-layer wiring, and its own duplicated controls:

| Map instance | File:line | How it's reached |
|---|---|---|
| `MainMapPage` (the real map) | `features/map_view/widgets/main_map_page.dart:58` | app home |
| `RoutePlanningPage` | `features/routing/widgets/route_planning_page.dart:44` | long-press → "Plan route" (`main_map_page.dart:437`) |
| `MeasuringMapPage` | `features/measuring/widgets/measuring_map_page.dart:59` | long-press → "Measure" (`main_map_page.dart:461`) |
| `RouteDrawingScreen` | `features/activities/widgets/route_drawing_screen.dart:42` | 6 activity create screens |
| `RegionCreationPage` | `features/tile_storage/offline_regions/widgets/region_creation_page.dart:50` | offline download |

`RoutePlanningPage` is the clearest case: it is a full `Scaffold` →
`MapBase` → fresh `MapController`, re-deriving base layers
(`route_planning_page.dart:78`), re-building attribution
(`:107-114`), and re-instantiating zoom/compass/location controls
(`:84-86`) — **all of which already exist on the map the user was just
looking at.** When the page opens, the map visibly *reloads*; the user's pan,
zoom-feel, and (critically) their **live location layer are gone** — the
planning map never mounts `CurrentLocationLayer`.

**Duplicated map chrome** across the 3 full-feature map pages (main, routing,
measuring): tile layers, `RichAttributionWidget` loops, and the
`defaultMapControls`/`defaultMobileMapControls` factory are each wired three
times.

**15+ `Navigator.push` / `MaterialPageRoute` sites** open full screens; the
four above replace the map context entirely rather than layering a tool onto it.

### Why it feels bad

Pushing a new map is a hard context switch: reload flash, lost camera state, no
live position, no continuity with the trails/markers the user was looking at.
A route planner *should* be a mode on the existing map, not a parallel universe.

### Maps to user complaint
> "Drawing and creating new paths opens new screens, instead of replacing the
> UI. It just feels bad."

---

## 3. Defect B — No overlay / popup / sheet coordinator

### Evidence

`MainMapPage` assembles a **flat list of `Positioned` overlays by hand**
(`main_map_page.dart:284-325`) with hard-coded, **colliding** bottom offsets:

| Overlay | Position | Visibility owner |
|---|---|---|
| `ModeIndicator` (follow/compass/nav chips) | `bottom:16` | `mode_indicator.dart:38` |
| `MarkerSelectionBar` | `bottom:0` | watches selection |
| `RecordingPanel` | `bottom:20` | watches recording |
| `DownloadProgressToolbar` | `bottom:24` | manual `_hiddenDownloadIds` set |

All four can be visible simultaneously and **overlap** — there is no slot
system, no z-order policy, no mutual exclusion. Each widget independently
returns `SizedBox.shrink()` when idle; none knows the others exist.

**58 imperative `showModalBottomSheet`/`showDialog` calls** across the app, with
**no open-sheet guard** anywhere. Nothing prevents a sheet stacking on a sheet
(tap marker → sheet → action → second sheet on top). There is **no central
overlay/modal manager** (confirmed absent from `lib/core` and `lib/app`).

Two **isolated** `OverlayEntry` systems exist for search dropdowns
(`search_bar_mobile.dart:34`, `search_bar_desktop.dart:33`) — self-managed,
unaware of the sheet layer, so a search dropdown can render *over* an open
sheet.

**Static snapshot bug** (the "static popups" complaint, literally): the pin
options sheet captures `ref.read(navigationStateProvider).isActive` at open time
and passes it as a plain bool (`main_map_page.dart:393`); if state changes while
the sheet is open, the sheet lies. (Most other sheets correctly `ref.watch`
inside the builder — so this is an inconsistency, not a universal rule.)

### Why it feels bad

Two panels fighting for `bottom:16-24`, a dropdown over a sheet, a sheet that
shows stale state — these are exactly the "multiple versions of widgets
overlaying" the user reported. Each was correct in isolation; nothing owns the
*composition*.

### Maps to user complaint
> "All of the popups and overlays are static, and we can end up with multiple
> versions of widgets overlaying — it feels really bad."

---

## 4. Defect C — Live-state silos (the integration failure)

This is the heart of the "disjointed" feeling. Four location-aware subsystems
exist; **none knows about the others**, and there is no shared concept of an
*active journey*.

| Subsystem | State it owns | File |
|---|---|---|
| **Follow mode** ("snap map to me") | `FollowMode {off,active,paused}` | `core/location/follow_mode_state.dart` |
| **Navigation** (point-to-point) | `NavigationState { target:LatLng?, isActive }` — *a single point, no path* | `features/navigation/data/navigation_state.dart:3` |
| **Recording** ("tracking") | `RecordingState { points, ascent, distance… }` — own 2 Hz sampler & distance engine | `features/path_recording/data/recording_notifier.dart` |
| **Routing** (generate route) | `RoutePlan { geometry, distanceM, durationS… }` — static, server-solved | `features/routing/data/route_planning_state.dart` |

**Proof of isolation:** no notifier reads another's provider. Both navigation
*and* recording independently call
`ref.read(followModeProvider.notifier).enable()`
(`navigation_state_notifier.dart:18`, `recording_notifier.dart:71`) — they each
poke follow mode but never coordinate. Distance is computed in **three**
separate places (`main_map_page.dart:204` for nav arrival,
`recording_notifier.dart:197` for track length, server-side for routes) — no
shared engine.

Three concrete integration gaps, each = a user complaint:

1. **Viewing a trail offers no action.** `TrailFeatureSheet`
   (`features/external_vector_layers/widgets/trail_feature_sheet.dart`) is
   **read-only** — name/difficulty/maintainer chips, zero buttons. There is no
   "follow this trail", "navigate along it", or "record while following".
   `SavedPath`'s `PathInfoSheet` has Edit/Export/Delete but still **no follow**.
   → *"When seeing paths, I should have the option to follow them."*

2. **Navigation can't follow a path.** `NavigationState` holds **one target
   point** and draws a straight line to it (`navigation_polyline_layer.dart`).
   There is no path-following navigation at all — so even if the trail sheet had
   a "follow" button, the navigation feature couldn't honor it without a rewrite.
   → *"This should integrate with the follow feature, not add a new one."*

3. **A generated route is a dead-end snapshot.** On `RoutePlanningPage` the route
   is a static polyline; the page **doesn't even show your live position**
   (no `CurrentLocationLayer`). The only thing you can do with a solved route is
   *Save as track* (`route_planning_sheet.dart:82`) — after which you must leave,
   manually start a recording, and manually start navigation. There is no
   "navigate/track along this route", no progress-along-route, no live position
   vs. route.
   → *"When I generate a route I should track it and my position, but it's
   static and my position isn't live — disjointed."*

### Maps to user complaint
> "Tracking dumbly opens a widget instead of being integrated… when I generate
> a route I should track this and my own position… my previous position is not
> live, it feels disjointed."

---

## 5. Defect D — Concept duplication without seams

### 5a. Five representations of "a polyline path"

| Concept | Type | File |
|---|---|---|
| Generated route | `RoutePlan` | `routing/models/route_models.dart:76` |
| Server trail tile | `MvtLayerSource` (no client model) | `curated_paths/models/…` |
| Saved path | `SavedPath` | `saved_paths/models/saved_path.dart:7` |
| Measured line | `MeasurePoint[]` | `measuring/models/measure_point.dart` |
| Recorded track | `RecordingResult` | `path_recording/models/recording_result.dart:6` |

They convert to one another **only by hand in UI code**: route→save, measure→save,
record→save each re-marshal fields into `SavePathSheet` at three different call
sites (`route_planning_sheet.dart:82`, the measuring page, `recording_panel.dart`).
There is no shared `Path` value type and no programmatic converters — so
"measure a line then record it," or "open a saved path and re-plan from it,"
are impossible without new bespoke code each time.

### 5b. Six copy-pasted activity features

`activity_hiking`, `activity_fishing`, `activity_backcountry_ski`,
`activity_xc_ski`, `activity_freediving`, `activity_packrafting` are
near-identical: each ships ~4 widgets + ~4 models + ~2 data files with the same
shapes (`<kind>_activity.dart`, `<kind>_create_screen.dart`,
`<kind>_detail_sheet.dart`, `descriptor.dart`). Adding a kind ≈ copy-pasting
10+ files; changing shared behavior ≈ editing 6 features.

**Good news (already composition-shaped):** `ActivityKindRegistry`
(`activities/data/activity_kind_registry.dart`) + per-kind `ActivityKindDescriptor`
(callbacks: `buildCreateScreen`, `buildDetailScreen`, `buildMapMarker`,
`buildMapPolyline`) registered once in `app/main.dart:33`. This is **exactly the
composition pattern we want** — the shell never imports a specific kind. The
duplication is *inside* the kinds because the descriptor doesn't yet compose
shared building blocks (a shared create-form scaffold, a shared detail sheet
shell, a shared serialization helper). The fix is **more composition** (shared
composable widgets the descriptors assemble), **not** an `Activity` base class.

### 5c. Layers & tools are hard-wired, not composed

Map layers are a hand-built list in `main_map_page.dart:264-282`; overlays a
hand-built list at `:284`. There is **no `MapLayerRegistry` / `MapToolRegistry`**
mirroring the activity-kind registry. So every new map feature edits the
god-widget — the opposite of the registry pattern that already works two
folders over.

### `api.dart` boundary — working, but a grab-bag

The `api.dart` facade is genuinely enforced (no feature reaches into another's
`data/`/`widgets/`). But each is an unsorted re-export pile (routing ≈ 25
exports mixing models, notifiers, providers, widgets, repository). Callers can't
tell "state to watch" from "component to render." Cosmetic vs. the above, but
worth grouping.

### Maps to user complaint
> "A ton of duplicated related features… none of them talk to each other or
> complement each other… huge missed opportunities."

---

## 6. Additional issues found (same class, not in the original list)

- **Live position is inconsistently present.** Mounted on the main map
  (`main_map_page.dart:270`) but **absent** from `RoutePlanningPage` and almost
  certainly the other pushed map pages — so the moment you enter a tool you lose
  yourself on the map.
- **`MapViewState` is camera-only** (`map_view/models/map_view_state.dart`:
  just `center` + `zoom`). There is no "active tool / mode" in shared state, so
  there is nowhere for mutual exclusion or a unified "cancel current tool" to
  live. This is the missing seam behind Defects A, B, and C at once.
- **Arrival/navigation logic lives in the god-widget's `build`.**
  `main_map_page.dart:199-212` hardcodes 15 m arrival detection via `ref.listen`
  — domain logic that belongs in the navigation notifier, leaking into the host.
- **Follow mode is enabled from two unrelated features** with no coordination
  (nav + recording), so who "owns" follow at any moment is ambiguous.
- **Tool state can't be mutually exclusive.** Nothing stops recording +
  measuring + navigating being "active" at once; the UI only avoids it by luck
  of which screen you're on.
- **Map controls rebuilt per page** instead of being a single composed control
  cluster bound to whichever map is active.

---

## 7. Target architecture — composition seams (not god modules)

The fix is **one composition layer + four seams**. None is a base class; each is
a registry/contract that features plug into. This keeps every feature
self-contained (per the existing architecture doc) while letting them
*compose on the map* and *compose with each other*.

### Seam 1 — One map, many mounted tools (`MapToolRegistry`)
Kill the pushed map pages. Keep **one** `MainMapPage` / one `MapController`.
A tool is a descriptor — `MapToolDescriptor { id, buildMapLayers(ctx),
buildOverlay(ctx), onActivate, onDeactivate }` — registered in `main.dart`
exactly like `ActivityKindDescriptor`. Route planning, measuring, route drawing,
region selection become **tools that mount layers + an overlay onto the live
map**, not screens that replace it. The live location layer, camera, and base
tiles are always there because you never left the map.

### Seam 2 — A map-layer registry (`MapLayerRegistry`)
Replace the hand-built list at `main_map_page.dart:264` with descriptors each
feature exports and registers. Adding a layer never edits the host again.

### Seam 3 — One overlay coordinator (slots + mutual exclusion)
A single `MapOverlayHost` owns named slots (`topCenter`, `bottomSheet`,
`bottomBanner`, `selectionBar`) and a z/priority policy. Tools/features submit
overlay content to a slot; the host decides what shows and prevents collisions.
Sheets go through one `showAppSheet`-style entry that enforces a single active
sheet (queue or replace). This dissolves Defect B.

### Seam 4 — A shared "active journey" state (the integration seam)
Introduce one composable provider — e.g. `ActiveJourney { route?, target?,
recordingTrack?, followPath? }` — that the existing features *read and write*,
rather than four silos:
- "Follow this trail" (trail sheet button) sets `followPath` → navigation draws
  the path, follow mode snaps to me, optional recording starts. **One action,
  three existing features compose.**
- "Track this route" on a solved route sets `route` as the active journey →
  live position, progress-along-route, and recording all key off the same state.
- A single distance/bearing/progress engine in `core/` replaces the three
  hand-rolled copies.

### Seam 5 — A `Path` value type + converters
A small shared `Path` (points + optional elevations + stats) in `core/`, with
`RoutePlan.toPath()`, `RecordingResult.toPath()`, `MeasurePoint[].toPath()`,
`SavedPath ↔ Path`. The three hand-conversions collapse into one, and
cross-feature flows (measure → record, saved → re-plan, route → track) become
trivial because everything speaks `Path`.

### Activities: compose, don't inherit
Keep the descriptor registry. Extract the *duplicated* parts of the 6 kinds into
**shared composable widgets** (a create-form scaffold, a detail-sheet shell, a
stats block) that each descriptor assembles with its kind-specific fields. No
`Activity` base class — the descriptor *is* the composition root.

---

## 8. Suggested sequencing (incremental, each shippable)

1. **Seam 4 (active journey) + `Path` type (Seam 5)** first — pure model/state,
   no UI churn, immediately enables "follow this trail" and "track this route".
2. **Seam 3 (overlay coordinator)** — fixes the visible collision/stacking mess
   with no behavior change.
3. **Seam 1 (map-tool registry)** — migrate route planning from a pushed page to
   an in-place tool as the pilot; then measuring, route drawing, region select.
4. **Seam 2 (layer registry)** — de-god-widget `MainMapPage`.
5. **Activity de-dup** — extract shared composable widgets behind the existing
   descriptors.

Each step removes code from `MainMapPage` and adds a seam; the god-widget
shrinks to a thin host that mounts the registries.

---

## Appendix — key file references

- God-widget host: `features/map_view/widgets/main_map_page.dart` (esp. 58, 184-212, 264-325, 437-469)
- Pushed map pages: `routing/widgets/route_planning_page.dart`, `measuring/widgets/measuring_map_page.dart`, `activities/widgets/route_drawing_screen.dart`, `tile_storage/offline_regions/widgets/region_creation_page.dart`
- Overlay/mode chips: `features/map_view/widgets/mode_indicator.dart`
- Live-state silos: `core/location/follow_mode_state.dart`, `features/navigation/data/navigation_state.dart`, `features/path_recording/data/recording_notifier.dart`, `features/routing/data/route_planning_state.dart`
- Read-only trail sheet: `features/external_vector_layers/widgets/trail_feature_sheet.dart`
- Path-type family: `routing/models/route_models.dart`, `saved_paths/models/saved_path.dart`, `measuring/models/measure_point.dart`, `path_recording/models/recording_result.dart`
- Working composition pattern to mirror: `activities/data/activity_kind_registry.dart` + `activity_*/descriptor.dart` + `app/main.dart:33`
- Architecture doc the implementation drifted from: `lib/context/architecture.context.md`
