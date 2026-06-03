# Cohesion follow-ups — implementation plan

**Date:** 2026-06-03
**Builds on:** the composition overhaul + the 4 cohesion tiers (journey, entity-action
registry, conditions seam, GeoPath capability) and the active-outing panel.
**Goal:** finish making the app feel like one product — live-following feels real,
no feature is an island, and complementary features (search, collections, offline,
conditions) plug into the same seams.

Principles unchanged: composition over inheritance; one state → one widget; reuse
the existing seams (`activeJourneyProvider`, `MapEntityActionRegistry`,
`GeoMetrics`, `GeoPath`) rather than add parallel ones.

---

## A. Live-following cluster  (items 1 + 2) — do first

The data already exists (`GeoMetrics.progress()` → `fraction`, `remainingM`,
`offRouteM`; `journeyRemainingMetersProvider`; `GeoMetrics.naismithSeconds`). This
is mostly rendering + a couple of listeners.

### A1. Progress bar + ETA in the outing panel  *(~small)*
- New derived provider `journeyProgressProvider` (journey feature) → returns a
  small record `{fraction, remainingM, etaSeconds, offRouteM}` computed from
  `activeJourney.path` + live `locationStateProvider` via `GeoMetrics.progress` +
  `naismithSeconds`. One source of truth; the panel + chip read it.
- `ActiveOutingPanel` following-header: add a `LinearProgressIndicator(value:
  fraction)` and an **ETA** line ("arrive ~14:32", local clock = now + eta).
- Recording-only (no followed path → no fraction): no bar; keep the distance/time
  stats already shown.
- Files: `features/journey/data/active_journey_notifier.dart` (provider),
  `features/journey/widgets/active_outing_panel.dart` (render).

### A2. Arrival + off-route for path-following  *(~small)*
- **Arrival**: in `ActiveJourneyNotifier.build()`, add `ref.listen(locationStateProvider)`
  — for `followingPath`, when `progress.remainingM < arrivalThresholdM` (~30 m),
  finish the outing: if recording, surface a save prompt (reuse the panel's
  `_stopAndSave`); else `stop()` + an "Arrived" snackbar. (Point-nav already has
  its 15 m auto-cancel — fold both into this one listener so arrival logic lives
  in one place, not split between the notifier and `main_map_page`.)
- **Off-route**: when `progress.offRouteM > offRouteThresholdM`, the panel shows an
  inline "Off route — re-route" affordance → one-tap `routingRepository.plan(points:
  [myLocation, journey.path.points.last])` (or remaining waypoints) → `followPath`
  with the new geometry, keeping `waypoints` for Edit. Debounce so it doesn't nag.
- Files: `active_journey_notifier.dart` (listener + thresholds), `active_outing_panel.dart`
  (off-route banner + re-route button), reuses `routingRepositoryProvider`.
- Note: move the arrival snackbar out of `main_map_page` into the journey layer so
  it's not duplicated.

---

## B. Retire the last islands  (items 3 + 4)

### B0. Add entity-specific actions to the action seam *(prereq, ~small)*
`MapEntityActionContext` today only has fixed optional callbacks. Add
`final List<MapEntityAction> extraActions;` so an entity can contribute its own
buttons (marker's *Save-as-activity*, *Photo*) into the **same** bar, appended
after the standard set. Keeps one bar, no bespoke `SheetActionBar`.
- File: `features/map_view/models/map_entity_action.dart`,
  `widgets/map_entity_action_bar.dart` (append `entity.extraActions`).

### B1. Markers → `MapEntityActionBar`  *(~medium-small)*
- Replace `marker_info_sheet.dart`'s hand-rolled `SheetActionBar` with
  `MapEntityActionBar(entity: MapEntityActionContext(point: marker.position, …))`.
- Standard actions light up: **Navigate · Conditions · Add-to-collection ·
  Edit · Delete**. (Markers get the weather/avalanche Conditions action they
  can't reach today.)
- Preserve marker-specific behaviour via `extraActions`: **Save-as-activity**
  (`_promoteToActivity`) and **Photo** (`_addPhoto`, non-web), plus **Export**
  via the `onExport` callback.
- **Preserve the navigate confirm-replace**: move the "already navigating to a
  different target → confirm before replacing" logic out of the marker sheet and
  into the standard `navigate` action (so every Navigate gets it). The action's
  invoke reads `activeJourneyProvider`; if a different journey is active it shows
  `AppDialog` then proceeds. One behaviour, centralised.
- Files: `marker_info_sheet.dart`, `map_entity_action_registry.dart` (navigate
  confirm), wire `onAddToCollection` to the existing `AddToCollectionSheet`.

### B2. Activity routes → Follow + stats + Conditions  *(~small-medium)*
- In the shared activity detail chrome (the `ActivityDetailScreen` /
  `activity_detail_chassis`), when the activity's geometry is a line
  (`ActivityGeometry.toGeoPath() != null`), render `MapEntityActionBar`
  (path capability → **Follow · Conditions · Save-as-track**) + `PathStatsPanel`.
  Point activities (fishing/freediving) get the point capability → Navigate +
  Conditions. Done once in the chassis, so all 6 kinds inherit it (composition,
  per the descriptor pattern).
- Files: `features/activities/widgets/detail/activity_detail_chassis.dart` (or the
  screen that owns the chrome), reuses `ActivityGeometryGeoPath`, action bar,
  `PathStatsPanel`.

---

## C. Structural finish — one selection model  (item 5)

The capstone: a single `selectedMapEntityProvider` + one detail-sheet host so
tapping any map entity is consistent and "tap empty coordinate → actions" is just
another selection.

- New: `features/map_view/data/map_selection.dart` —
  `MapSelection { GeoPath? path; LatLng? point; String title; … callbacks }`
  (essentially a persisted `MapEntityActionContext` source) + provider.
- Each layer's tap handler (trail vector, saved path, marker, activity, photo)
  sets the selection instead of each opening its own sheet directly. A single
  `MapEntityDetailHost` overlay renders the right body + the `MapEntityActionBar`.
- Long-press empty map → a coordinate selection (point only) → Navigate /
  Conditions / Create marker / Plan route — replacing the bespoke `PinOptionsSheet`
  with the same host.
- Risk: medium — touches every layer's hit-testing. Do AFTER A + B so the action
  bar/behaviours are already proven; migrate one layer at a time behind the host.
- Files: new selection model + host in `map_view`; per-feature tap handlers updated
  to `ref.read(selectedMapEntityProvider.notifier).select(...)`.

**Status (done):** the seam is built and shipped — `MapSelection`
(`models/map_selection.dart`), `selectedMapEntityProvider`
(`data/map_selection_notifier.dart`), the generic `MapEntityDetailSheet`
(body + shared `MapEntityActionBar`) and the zero-footprint `MapEntityDetailHost`
overlay (registered in `app/map_overlays.dart`). The **long-press coordinate
flow** is fully migrated onto it: `MainMapPage._selectCoordinate` selects a
coordinate `MapSelection` (rich `CoordinateDetailBody` = place-info + weather,
`includeStandardActions: false`, its own Navigate/Marker/Activity/Measure/Route
actions — preserving the route-from-my-location auto-follow). The bespoke
`PinOptionsSheet` is deleted. A new `includeStandardActions` knob on
`MapEntityActionContext` lets a selection bring its own complete action set.
**Incremental remainder:** the per-layer entity *tap* handlers (marker / saved
path / trail / activity) still open their own rich sheets — those sheets already
render the shared `MapEntityActionBar`, so actions are already consistent; moving
their *presentation* behind the host is the remaining one-layer-at-a-time work,
safe to do later behind the now-proven seam.

---

## D. Cross-feature integration  (items 6 + 7)

### D1. Search → select → act  *(~small, mostly additive)*
Reality: `CompositeSearchService` already federates marker + path + trail +
place-name backends. Remaining work:
- Add an **activities** backend (`activity_search_service.dart`) to the composite
  (mirrors `marker_search_service` / `path_search_service`).
- On result tap: instead of only panning, **select** the underlying entity
  (via C's `selectedMapEntityProvider`) so the action bar appears — search becomes
  the universal entry point. (Until C lands, pan + open the entity's existing sheet.)
- Files: `features/search/data/composite_search_service.dart`,
  new `activity_search_service.dart`, result-tap handler in the search bars.

### D2. Planned-vs-actual  *(~medium — has a schema change)*
- Add `GeoMetrics.deviation(actual, planned) → {completionPct, avgOffsetM, maxOffsetM}`
  (project each actual point onto the planned line via the existing `progress`
  machinery).
- Persist the link: add `plannedGeometry: List<double[]>?` (or `routeWaypoints`)
  to `SavedPath` (+ `toLocalMap`/`fromLocalMap` + a SQLite column + a tiny
  migration). When the outing-panel save fires *while following a route with
  waypoints*, pass the planned geometry through `SavePathSheet`/`GeoPath` into the
  saved track.
- `PathStatsPanel` (or path detail) shows "completed 92% · max 40 m off" + draws
  the planned line faintly under the actual track when both exist.
- Files: `core/geo/geo_metrics.dart`, `saved_paths/models/saved_path.dart` (+ store
  + migration), `active_outing_panel.dart` (pass planned geometry on save),
  `path_stats_panel.dart` (deviation row).
- Risk: medium — the DB migration is the only non-trivial part; keep the column
  nullable so old rows are fine.

---

## E. Bigger plays (only if prioritised)

### E1. Collections as map state  *(~medium)*
- `collectionVisibilityProvider` (set of "shown on map" collection ids); the layer
  registry filters markers/paths to shown collections. "Show on map" toggle on the
  collection detail.
- A collection as a **trip**: order its paths and offer "Follow trip" → a journey
  over the concatenated `GeoPath`s. Make activities + photos collectable via the
  universal Add-to-collection action (B0).
- Files: `collections` (visibility provider + trip builder), layer descriptors read
  visibility, journey consumes the concatenated path.

### E2. Offline-along-route  *(~medium)*
- In the active-outing panel (and the route planner), an "Download offline"
  action → seed the `RegionSelectionNotifier` with a **corridor** (buffer the
  followed/planned `GeoPath` into a bounds/polygon) and open the existing
  `DownloadDetailsSheet`. Ties `journey`/`routing` → `tile_storage` via the seam
  that already exists.
- Files: `offline_regions` (corridor-from-path helper), a Download action on the
  outing panel / planner.

**Status (done):** `corridorBounds(GeoPath)` helper in
`offline_regions/data/route_corridor.dart` (padded bbox of the path, exported
from the offline api alongside `DownloadDetailsSheet`); a "Download map along
route" icon button in `ActiveOutingPanel`'s following header opens
`DownloadDetailsSheet(bounds: corridorBounds(journey.path))`.

---

## Status summary (2026-06-03)

All of A–E landed and are analyze-clean + green (760 tests pass, 4 skipped; the
only intermittent red is the pre-existing real-isolate `download_orchestrator`
integration test, which passes in isolation/its own dir and is orthogonal to
this work):
- **A** live-following, **B** markers+activities action bar, **C** selection
  seam + coordinate flow, **D1** search-select + activities backend, **D2**
  planned-vs-actual (DB v15 `planned_geometry` + `GeoMetrics.deviation` +
  PathStatsPanel row), **E1** collection visibility (pre-shipped) + "Follow
  trip", **E2** offline-along-route.

Incremental remainders (deliberately deferred, safe to do later):
- **C**: migrate the per-layer entity *tap* handlers behind the detail host
  (their sheets already share the action bar, so actions are already consistent).
- **D2**: `plannedGeometry` is local-only and may be lost on a server sync
  round-trip; draw the planned line faintly under the actual track on the path
  detail map.
- **E1**: make activities + photos collectable via the universal
  Add-to-collection action.

---

## Sequence & rationale
1. **A1 + A2** — live-following cluster. Highest felt impact, data already there,
   self-contained in the journey feature. (Your explicit ask.)
2. **B0 → B1 → B2** — retire the marker + activity islands via the existing action
   bar; B0 (extraActions) unblocks B1.
3. **C** — selection model + detail host (structural capstone; do once A+B prove
   the action/behaviour set).
4. **D1** then **D2** — search-select (cheap, leans on C) then planned-vs-actual
   (schema change).
5. **E1 / E2** — only if collections/offline are product priorities.

Each step: `flutter analyze` clean + `flutter test` green before the next, and a
device check of the live-following + tap flows (the parts unit tests can't cover).
