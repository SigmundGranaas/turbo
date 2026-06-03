# Composition Overhaul — Implementation Plan

**Date:** 2026-06-02
**Companion to:** `2026-06-ux-integration-and-composition-audit.md`
**Goal:** Fix all four defects from the audit (screen proliferation, overlay
chaos, live-state silos, concept duplication) by introducing a **composition
layer + five seams**. No god modules, no base classes — every feature stays
self-contained behind its `api.dart` and *plugs into* shared seams.

---

## Design rules (non-negotiable)

1. **Composition over inheritance.** Seams are registries and value types that
   features *plug into*, never base classes they extend. The proven model is the
   existing `ActivityKindRegistry` (`Provider<Registry>` overridden in
   `app/main.dart`, iterated by a shell that never names a concrete kind). Every
   new seam copies this exact shape.
2. **One map, ever.** Tools mount onto the single `MainMapPage` `MapController`.
   No feature creates a second `FlutterMap`/`MapController`.
3. **`api.dart` boundary stays.** Shared value types live in `core/`; features
   expose converters from their own `api.dart`. No feature reaches into another's
   internals (current state — keep it).
4. **Every phase is independently shippable** and *removes* code from
   `MainMapPage`. The god-widget shrinks to a thin host that mounts registries.

---

## Phase 1 — `GeoPath` value type + converters  *(model only, no UI churn)*

**Problem solved:** Defect D-5a (five siloed path representations, hand-conversion
at 3 sites).

**New:** `core/geo/geo_path.dart`
```dart
enum GeoPathSource { route, recording, measure, saved, trail }

/// Canonical "a line on the map" value type. Every feature converts to/from
/// this; nothing else crosses feature boundaries to represent a path.
class GeoPath {
  final List<LatLng> points;
  final List<double?>? elevations;   // canonical nullable-per-point form
  final double distanceM;
  final double? ascentM;
  final double? descentM;
  final int? movingTimeSeconds;
  final GeoPathSource source;
  // bounds getter, isEmpty, copyWith
}
```
Plus `core/geo/geo_metrics.dart` — the **single** distance/bearing/ETA/
progress-along-path engine that replaces the three hand-rolled copies
(`main_map_page.dart:204`, `recording_notifier.dart:197`, server-side display).

**Converters** (extensions, each exported from the owning feature's `api.dart`):
- `RoutePlan.toGeoPath()` — `route_models.dart` (`geometry`→points, `distanceM`,
  `ascentM`; elevations null).
- `RecordingResult.toGeoPath()` — already `List<double?>` elevations; direct.
- `SavedPath.toGeoPath()` / `GeoPath.toSavedPath({title,…})` — reconcile
  `List<double>?` ↔ `List<double?>?`.
- `measurePointsToGeoPath(List<MeasurePoint>)` — measuring feature.

**Migration:** rewrite the 3 `SavePathSheet` call sites
(`route_planning_sheet.dart:82`, measuring page, `recording_panel.dart`) to go
`X.toGeoPath() → GeoPath.toSavedPath(...)`. Behaviour identical; conversion now
lives in one place and cross-flows (measure→record, saved→re-plan) become trivial.

**Verify:** `flutter test` (add `geo_path_test.dart` round-trip + metrics unit
tests); `flutter analyze`. No visible change yet.

---

## Phase 2 — Active-journey state  *(the integration seam)*

**Problem solved:** Defect C (follow/navigate/record/route are 4 unaware silos;
routes static; position not live; trails offer no action).

**New module:** `features/journey/` (own `api.dart`). It does **not** own GPS or
map-snap — it *orchestrates* the primitives that already exist.

```dart
enum JourneyKind { none, navigatingToPoint, followingPath, recording }

class ActiveJourney {
  final JourneyKind kind;
  final LatLng? target;        // point nav
  final GeoPath? path;         // trail/route being followed (Phase 1 type)
  final bool recording;        // is a live track being captured
  // derived live values come from geo_metrics + locationStateProvider:
  //   distanceRemaining, bearing, progressPct, offRouteM, etaSeconds
}

final activeJourneyProvider = NotifierProvider<ActiveJourneyNotifier, ActiveJourney>(…);
// API: followPath(GeoPath, {record}), navigateToPoint(LatLng), startRecording(),
//      stop(); internally calls followModeProvider + recordingNotifier.
```

**Wiring (compose existing features, delete duplication):**
- `navigation_state_notifier.dart`: `NavigationState` becomes a thin projection of
  `ActiveJourney` (target = `journey.target`), or navigation delegates to the
  journey. The inline 15 m arrival logic moves out of `main_map_page.dart:199-212`
  into the journey notifier (uses `geo_metrics`).
- `followModeProvider` stays the **map-snap primitive**; only the journey enables
  it now (remove the duplicate `enable()` calls scattered in nav + recording —
  the journey owns "should the map follow me").
- `RecordingNotifier` registers its live track with the journey (recording becomes
  a *facet* of a journey, not an island); its own distance math swaps to
  `geo_metrics`.

**New shared layer:** `JourneyLayer` (live position vs. path/target, progress) —
mounts on the **main** map and on any tool (Phase 4) so position is *always* live.

**UI payoff (small, high-impact edits):**
- `TrailFeatureSheet` (currently read-only) gains **"Follow this trail"** →
  `activeJourney.followPath(trail.toGeoPath(), record: optional)`. One action,
  three existing features compose.
- `RouteResultSheet` gains **"Track this route"** → `followPath(plan.toGeoPath())`.
- `PathInfoSheet` (saved paths) gains **"Follow"**.

**Verify:** `flutter test` (journey state machine, arrival, progress); manual:
tap trail → Follow → live dot + progress; generate route → Track → live position.

---

## Phase 3 — Overlay coordinator  *(no behaviour change, kills the visual mess)*

**Problem solved:** Defect B (flat colliding `Positioned` list; 58 unguarded
sheets; dropdown-over-sheet; static snapshot).

**New:** `features/map_view` → `MapOverlayHost` + slot model.
```dart
enum OverlaySlot { topCenter, bottomBanner, selectionBar, bottomSheetAnchor }

class OverlayContribution {
  final OverlaySlot slot;
  final int priority;          // higher wins / stacks deterministically
  final WidgetBuilder builder; // built with ref → reactive, never snapshotted
}
final mapOverlayProvider = NotifierProvider<…, List<OverlayContribution>>(…);
```
`MapOverlayHost` lays out contributions per slot with a **collision/stacking
policy** (one occupant per exclusive slot; deterministic vertical stacking where
allowed) and feeds the resulting widgets into `MapBase.overlayWidgets`. This
replaces the hand-built list at `main_map_page.dart:284-325`.

**Single-sheet policy:** one `showAppSheet(...)` entry point backed by an
`openSheetProvider`; opening while one is active **replaces or queues** (no
stacking). Migrate the 58 `showModalBottomSheet`/`showDialog` calls to it
incrementally (start with `main_map_page` + markers).

**Fix the snapshot bug:** `_showPinOptionsSheet` (`main_map_page.dart:393`) — move
`isNavigating` inside the builder as `ref.watch`.

**Search overlays** (`search_bar_*.dart`) register through the same host so a
dropdown can't render over a sheet.

**Verify:** `flutter test` (slot exclusion, single-sheet); manual: record +
select markers + active download simultaneously → no overlap.

---

## Phase 4 — Map-tool registry  *(kills the extra map instances)*

**Problem solved:** Defect A (5 map instances; pushed full-screen tools).

**New:** `features/map_view` → tool seam (mirror `ActivityKindRegistry` exactly).
```dart
class MapToolDescriptor {
  final String id; final String label; final IconData icon;
  List<Widget> buildLayers(MapToolContext ctx);     // merged into the live map
  Widget? buildOverlay(MapToolContext ctx);          // via overlay host (Phase 3)
  InteractionOptions? interactionOverride;           // e.g. freeze pan while dragging
  void onActivate(Ref) / onDeactivate(Ref);
  Future<void> Function(LatLng)? onMapTap;           // tool consumes taps
}
final mapToolRegistryProvider = Provider<MapToolRegistry>((_) => MapToolRegistry(const []));
final activeMapToolProvider = NotifierProvider<…, String?>(…); // ONE tool at a time
```
Registered in `app/main.dart` next to `activityKindRegistryProvider`.

`MainMapPage` reads `activeMapTool`, merges `tool.buildLayers()` into
`commonMapLayers`, routes `onTap`/`interactionOptions` to the tool, and shows
`tool.buildOverlay()` via the overlay host. `MapBase` already supports all of
this (`interactionOptions`, `onTap`, layer/overlay lists) — no new map primitive.

**Migrate, one tool at a time (pilot = route planning):**
1. **Route planning** → `routePlanningTool`. Reuse `RoutePlanningSheet`,
   `RoutePolylineLayer`, `RouteWaypointDot`, `route_planning_notifier` as-is; they
   become the tool's overlay + layers. **Delete** `RoutePlanningPage`'s
   `Scaffold` + `MapController` + duplicated tiles/attribution/controls
   (`route_planning_page.dart`). The drag logic (`_onWaypointDrag`) moves into the
   tool, using the shared controller. `main_map_page.dart:437` switches from
   `Navigator.push` to `activeMapTool = 'route_planning'`.
2. **Measuring** → `measuringTool` (delete `MeasuringMapPage`'s map).
3. **Route drawing** (activities) → `routeDrawingTool` (delete its `MapController`;
   the 6 activity create screens trigger the tool instead of pushing a screen).
4. **Offline region selection** → `regionSelectTool` (delete `RegionCreationPage`'s
   map; keep the management list page — that one's a legitimate list screen).

After migration: **1 `MapController` total** (down from 5); live position + base
tiles + trails persist into every tool because you never leave the map.

**Verify:** `flutter analyze`; manual per tool: enter tool → map keeps camera +
live dot; exit → returns cleanly; only one tool active at a time.

---

## Phase 5 — Map-layer registry  *(de-god-widget the host)*

**Problem solved:** Defect D-5c (layers hard-wired; every new feature edits the
host).

**New:** `MapLayerDescriptor { id, build(ctx), defaultVisible }` + registry
provider overridden in `main.dart`. Each layer feature (recording trace, saved
paths, navigation/journey, trails, ocean, activities, photos, markers) exports a
descriptor. `MainMapPage`'s `commonMapLayers` (`:264-282`) becomes
`registry.all.map((d) => d.build(ctx))`. Adding a layer never touches the host.

`MainMapPage` is now a thin host: mount layer registry + tool registry + overlay
host. The ~20 direct feature imports collapse to registry iteration.

**Verify:** `flutter analyze`; manual smoke of every layer's visibility toggle.

---

## Phase 6 — Activity de-duplication via shared composable widgets  *(optional, last)*

**Problem solved:** Defect D-5b (6 copy-pasted activity features).

Keep the descriptor registry (already correct composition). Extract the
*duplicated* parts of the 6 kinds into **shared composable widgets** in the
`activities` feature: `ActivityCreateScaffold`, `ActivityDetailSheetShell`,
`ActivityStatsBlock`, a shared serialization helper. Each `*_create_screen` /
`*_detail_sheet` becomes a thin composition of these + kind-specific fields.
**No `Activity` base class** — the descriptor remains the composition root.

**Verify:** `flutter test` per kind; visual parity check on each create/detail.

---

## Sequencing rationale

| Phase | Risk | Visible change | Unblocks |
|---|---|---|---|
| 1 `GeoPath` | very low | none | all path flows + journey |
| 2 Journey | medium | **"follow trail" / "track route" + live position** | the core complaint |
| 3 Overlay host | low | overlap mess gone | tool overlays |
| 4 Tool registry | medium-high | **no more new-screen flashes** | one-map UX |
| 5 Layer registry | low | none (internal) | future features |
| 6 Activity de-dup | low | none | maintainability |

Phases 1-3 deliver the integration wins with little UI risk; Phase 4 is the
biggest behavioural change (do route-planning as the pilot, validate, then the
other three tools); 5-6 are cleanup that pays off long-term.

## Risks & mitigations
- **Phase 4 gesture conflicts** (tool drag vs. map pan): `MapBase` already proved
  the pattern in `RoutePlanningPage` (freeze interaction while dragging via
  `interactionOverride`); port it, don't reinvent.
- **Journey ↔ recording ownership**: recording stays a standalone notifier;
  journey only *references* it, so background recording survives journey changes.
- **Incremental sheet migration**: the single-sheet entry can coexist with raw
  `showModalBottomSheet` during migration; convert call sites in batches.
- **No regression in saved-path sync**: Phase 1 only adds converters; `SavedPath`
  storage/sync contract is untouched.

## Decision: route-drawing stays a focused sub-editor (not an in-place tool)

`RouteDrawingScreen` (activity wizards) is the one remaining pushed screen with
its own `MapController`. After review it is **kept as a pushed sub-editor**, not
migrated to a `MapToolDescriptor`, because:

- It is launched **from within a create *form*** (a pushed `Scaffold` holding
  unsaved name/description/stats) and **returns a `List<LatLng>`** to that form.
  An in-place map tool can't return a value, and to draw on the *main* map the
  form would have to be torn down — losing the user's in-progress input unless
  the whole draft is lifted into a provider and the form re-presented. That's a
  wizard-flow redesign with real regression risk, for no on-map-stacking gain.
- It is **not** the problem the overhaul targeted: it doesn't stack competing
  widgets on the live map; it's a single full-screen modal editor with a clear
  return value and a preserved parent form. "Drawing opens a new screen" is
  acceptable *here* (a wizard step) in a way it wasn't for route-planning /
  measuring (top-level map modes), which were migrated.

If we later make the activity create form itself a bottom sheet over the map,
folding draw-on-map in becomes natural; until then the sub-editor is the right
shape. Tracked as a conscious decision, not an omission.

## Hardening pass (overlays + state-combining), 2026-06-02

- **Overlay registry** — `MapOverlayDescriptor`/`MapOverlayRegistry`
  (`mapOverlayRegistryProvider`), composed in `app/map_overlays.dart`. Selection
  bar, download toolbar, recording panel and the status chip are now descriptors
  in slots (`bottomBar` / `bottomFloating`); `MainMapPage` no longer hand-places
  them. Hidden-download state moved to `hiddenDownloadsProvider`.
- **Combined status chip** — the up-to-four stacked chips (journey, point-nav,
  follow, compass) collapsed into ONE adaptive `ModeIndicator` chip: dominant
  destination (journey/nav) → follow → compass-only, with the compass heading
  folded in as an inline badge. *Combining states instead of stacking widgets.*
- **Search ↔ modal coordination** — `SearchDismissObserver` (a
  `NavigatorObserver` on the root `MaterialApp`) + `TransientSurface` dismiss the
  search dropdown whenever any sheet/dialog/page is pushed, so a transient
  overlay can't sit over a modal — covering all call sites via one mechanism
  rather than editing ~30 sheets. **Deliberately did NOT** force all sheets
  through the single-sheet guard: many are intentional sheet-from-sheet flows
  (pick image source within marker-create, pick collection within save-path)
  where the parent must stay to receive the child's result.

## Out of scope
- Backend/routing-solver work (separate track, tasks #103-#171).
- `api.dart` semantic re-grouping (cosmetic; can follow later).
- New product features — this is consolidation of what exists.
