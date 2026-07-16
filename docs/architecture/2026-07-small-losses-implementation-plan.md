# Implementation plan: the seven "smaller losses" from the Flutter migration

**Date:** 2026-07-16.
**Source:** `docs/architecture/2026-07-feature-matrix.md` — the small Flutter features with no
successor. Reference designs extracted from the deleted Flutter app at `4b25ebb1^`; backend and
client seams surveyed at HEAD.

Features: (1) address/kommune search, (2) track line styling, (3) marker export, (4) custom tile
URLs, (5) elevation backfill, (6) location-marker customization, (7) friends/groups.

---

## Discoveries that reshape the plan

The naive reading of the feature matrix overstates the work. Three findings from the survey:

1. **The friends/groups backend fully exists.** Every endpoint the Flutter client called —
   `/api/sharing/friendships` (request/accept/block/remove), `/groups` (create/rename/members),
   `/grants`, `/invites`, `/me/profile`, `/users/lookup` — is implemented and wired in
   `apps/api/src/Sharing/` (EF services, no stubs). Moreover the **Android client is already fully
   implemented** — `KtorSharingRepository` covers all of it; the "unsupported" returns are only
   interface *defaults* for test fakes. Feature 7 is therefore: verify end-to-end, build the web
   UI, and add the one genuinely missing piece — a **visibility-change endpoint** (the
   `private/friends/unlisted_link/public` model exists on `Resource`, but nothing calls
   `ChangeVisibility`).
2. **Track style fields already ride the wire.** Server `TrackEntity` has `ColorHex`, `IconKey`,
   `LineStyleKey`, `Smoothing`; Android's sync DTO (`TrackMetadataDto`) already carries them; the
   web editor already edits colour and the web renderer already draws per-track colour. The only
   missing links are Android's local model (`SavedPath`/Room) and its detail-screen UI + tube
   colour threading.
3. **Elevation endpoints already exist.** Tileserver serves `POST /v1/elev/sample` (point) and
   `POST /v1/elev/profile` (polyline, uniform resample). Backfill needs no Kartverket dependency —
   only a small batch **per-vertex** endpoint (`Dem::sample` in a loop) plus client import hooks.

## Phasing overview

| Phase | Features | Backend work | Est. size |
|---|---|---|---|
| **P0 — client-only quick wins** | 3 marker export · 2 track styling (Android) · 6 location marker | none | 3 small slices |
| **P1 — elevation backfill** | 5 | tiny (one tileserver route) | small |
| **P2 — search breadth** | 1 address + kommune | none (Geonorge direct) | medium |
| **P3 — custom tile URLs** | 4 | none | medium |
| **P4 — social** | 7 friends/groups + visibility | one endpoint + web UI | medium |

Each phase is independently shippable; order within P0 is arbitrary. Estimates assume the
repo's existing test conventions (behaviour-rooted unit tests; emulator QA for Android
gesture/visual surfaces; vitest for web; cargo tests for tileserver).

---

## P0.1 — Marker export (Android + web)

**Goal:** export/share markers as files, matching Flutter (GeoJSON `FeatureCollection` of `Point`
features with `title/description/icon` properties) plus GPX `<wpt>` for interop.

**Design.** Pure client serializers over the existing marker models; reuse each platform's track
export plumbing (format enum, file naming, share/download).

**Android**
- New `MarkerExport.kt` beside `Gpx.kt`: `serializeMarkers(markers, format)` for
  `ExportFormat.Gpx` (one `<wpt lat lon><name><desc><sym>` per marker) and `GeoJson`
  (FeatureCollection; only non-empty props). KML optional — skip unless free.
- Entry points: single marker (detail sheet in `MapScreenParts.kt` — add an Export row beside the
  existing actions) and all markers of a collection (collection screen). Reuse the
  `shareTrack()`-style cache-file + `Intent.createChooser` plumbing from `PathsScreen.kt:492`.
- Tests: serializer round-trip behaviour (import GPX wpt back? we don't import waypoints — assert
  well-formedness + field mapping against small fixtures).

**Web**
- `features/markers/api.ts`: add `serializeMarkers(markers, fmt)` mirroring
  `serializeTrack` (`toGpxWaypoints`, `toGeoJsonPoints`); download via the existing blob helper.
- Entry points: marker detail panel (Export button) and collection detail (Export all).
- Tests: vitest serializer tests.

**Out of scope:** Flutter's share-as-link codec for a single marker (share links already exist via
the grants system).

## P0.2 — Track line styling (Android; web dash exposure)

**Goal:** per-track colour (10-colour palette, Flutter's `pathColorPalette`) and line style
(solid/dotted/dashed/dash-dot) that persists, syncs, and renders. Server + web colour are done.

**Android**
1. Model: add `colorHex`, `iconKey`, `lineStyleKey` to `SavedPath`; add columns to `PathEntity`
   with a Room migration in `MarkerDatabase.kt`; extend the four mappers
   (`PathEntity.toDomain`, `SavedPath.toEntity`, `PathEntity.toWriteRequest` — currently sends
   name only — and `TrackResponseDto.toEntity` — currently drops them). This immediately fixes a
   latent sync bug: a colour set on web is currently *discarded* by Android's sync.
2. UI: colour swatch row + line-style segmented control in `PathDetailScreen` (mirror web's
   `TrackEditorPanel` and Flutter's `path_customization_controls`). Save through
   `PathsViewModel` → `PathRepository.save` (dirty → sync push).
3. Render: thread `colorHex` to `controller.setRouteTube("track", track, color)` at
   `TurbomapMapView.kt:239` (the signature already takes colour — today it passes the hardcoded
   `TrackColor`).
4. Dash on tubes: the `Tube` scene layer has no dash field. **Phase A ships colour only** (style
   picker stores `lineStyleKey` but tube renders solid). **Phase B (optional)**: either add a dash
   pattern to the Rust `Layer::Tube` IR (engine work: shader param along arc-length) or render
   styled tracks as draped `line` layers (which already support `dash_array`) at the cost of the
   3D tube look. Decide after Phase A feedback — colour covers most of the differentiation value.
- Tests: mapper round-trips (entity↔domain↔wire, colour survives), migration test, sync
  request-fidelity (colour included in `toWriteRequest`).

**Web**
- Optionally expose line style: `MapLine` already supports `dashed`; add a style row to
  `TrackEditorPanel` writing `lineStyleKey`, map `dashed|dotted|dash_dot → dash_array` variants in
  `scene.ts appendContent`. Small; do together with the Android picker so the `lineStyleKey`
  vocabulary (`solid/dotted/dashed/dash_dot`, Flutter's keys) is fixed once.

## P0.3 — Location-marker customization (Android + web)

**Goal:** user-selectable my-position appearance. Flutter offered icon/photo, size, heading arrow
toggle + colours. Scope for parity-that-matters: **dot colour, size (0.75–1.5×), heading beam
toggle** on Android; **dot colour** on web. Custom photo icons: cut (low value, heavy).

**Android**
- Settings: add `locationDotColorArgb: Int?`, `locationMarkerScale: Float`,
  `showHeadingBeam: Boolean` to `UserSettings` + `SettingsRepository`/DataStore keys + rows in
  `SettingsScreen` (colour picker reuses the track palette; slider; switch).
- Rendering: `MapOverlay(...)` gains `userDotColor: Color`, `userMarkerScale: Float`,
  `showHeading: Boolean` params (defaults preserve today's look); `MyPositionPin` and
  `OffScreenPositionChevron` drop the hardcoded `UserBlue`. `TurbomapMapView` reads settings and
  threads them (settings flow → composable state).
- Tests: overlay renders with injected colour (existing MapOverlay test patterns); settings
  round-trip.

**Web**
- `uiStore`: `locationDotColor?: string` (persisted); `scene.ts appendContent` user-fix block
  parameterized (replace `LOCATION_BLUE` const with content-plane field on `userFix` or store
  read); a colour row in `AccountSettingsPanel` map section.

## P1 — Elevation backfill (tileserver + both clients)

**Goal:** imported tracks without `<ele>` get elevation profiles; ascent/descent recomputed.
Self-hosted DEM replaces Flutter's Kartverket høydedata dependency (better: no rate limits, same
data as the 3D terrain).

**Backend (tiny):** `POST /v1/elev/samples` in `turbo-tiles-api/src/v1/elev.rs` — body
`{points: [[lon,lat],...]}` → `{elev_m: [f32|null, ...]}`, per-vertex (unlike `/profile`'s uniform
resampling), implemented as a loop over the existing `Dem::sample`. Cap request size (e.g. 4 000
points). Cargo test against the existing DEM fixture.

**Clients (both, same shape):**
- Trigger on import when ≥ 50 % of points lack elevation and `points.length ≤ 4000` (Flutter's
  thresholds, relaxed cap since our endpoint is batched): fetch missing vertices in one call
  (chunk at the cap), fill the sparse list, recompute ascent/descent through the existing
  hysteresis helpers (`GeoMetrics.gainLoss` / `trackStats`), save, then sync.
- Android: hook right after `TrackImport.parse()` in the import flow, before
  `PathRepository.save`; new `ElevationRepository` in `core/data` hitting the tileserver. Note
  Android's wire push only sends elevations when complete — backfill makes previously-partial
  tracks syncable.
- Web: hook in `PathsListPanel` import flow after `parseTrack`, before create; show a "filling
  elevation…" toast (Flutter showed a snackbar).
- Non-goal: backfilling *recorded* tracks (GPS altitude now MSL-correct on Android) or already-
  synced historical tracks (could be a later batch job).

## P2 — Address + kommune search (Android + web)

**Goal:** the search bar finds street addresses and municipalities, as Flutter's composite search
did. Protected areas: defer (place-core's embedded bundle already has the logic; it's a bigger
data question).

**Design decision — call Geonorge directly from clients** (what Flutter did; the endpoints are
CORS-friendly since Flutter-web used them):
- Addresses (forward): `GET https://ws.geonorge.no/adresser/v1/sok?sok=<q>&treffPerSide=5` →
  `adressetekst` + `postnummer poststed`.
- Kommune (forward): `GET https://ws.geonorge.no/kommuneinfo/v1/sok?knavn=<q>*` →
  `kommunenavn`, `fylkesnavn`, `punktIOmrade` centre.
- Rationale: zero backend work now; the long-term home (Matrikkel ingest into the tileserver
  anchor index / places pipeline) is already on the places roadmap and can transparently replace
  the client backends later. Revisit if Geonorge availability becomes a problem.

**Android**
- `SearchResultType` gains `Address`, `Kommune`; new small repositories (mirror
  `KartverketSearchRepository`) injected into `SearchViewModel`; add to the existing concurrent
  fan-out; extend filter chips (`All / Markers / Places / Addresses`); icons via existing kind
  mapping. `SearchHit` gains an optional `kind` so address vs place render distinctly.
- Tests: ViewModel fusion behaviour with faked repositories (result ordering, filter chips).

**Web**
- `api/places.ts` gains `searchAddresses(q)` + `searchKommuner(q)` (direct Geonorge, same DTO
  massaged into `PlaceHit` with new `kind` values `address`/`kommune`); MapScreen merges the three
  fetches (places index + the two Geonorge calls) into `results` with type-grouped rendering and
  icon per kind.
- Tests: vitest for the response mappers.

## P3 — Custom tile URLs (Android + web)

**Goal:** user-supplied XYZ raster basemaps/overlays, as Flutter's "add custom map" offered.
**Scope: XYZ templates only** — Flutter also accepted WMS, but the wgpu engine's raster source is
an XYZ template; WMS would need engine work. Validation mirrors Flutter: http(s) + `{z}`/`{x}`/`{y}`
placeholders required; name; optional max zoom (default 19).

**Android**
- Model `CustomTileSource(id, name, urlTemplate, maxZoom, asOverlay: Boolean)` persisted as a JSON
  list in DataStore (`SettingsRepository`; kotlinx-serialization).
- `MapStyles`: `turbomapRasterSpecs`/`baseTiles` accept the custom list — selection model changes
  from the closed `BaseLayer` enum to `BaseLayerId: String` internally (enum ids + `custom_<uuid>`),
  keeping the enum for built-ins. `MapLayersSheet`: dynamic cards for custom sources + an
  "Add map…" card opening a small form dialog (name, URL, live validation hint) + long-press to
  delete.
- Offline note: `WgpuOfflineTileManager` builds lanes from specs — custom sources participate in
  viewport downloads automatically once they flow through `turbomapRasterSpecs`; verify quota
  behaviour.
- Tests: validation unit tests; settings round-trip; layer-sheet state.

**Web**
- `baseLayers.ts`: widen `BaseLayerId` to `string`; `uiStore` gains `customLayers:
  {id,label,url,maxZoom}[]` (persisted); `buildBaseScene`/`LayerPicker` fall back to the custom
  list (`buildBaseScene` already takes any URL). "Add map…" row in `LayerPicker` with inline
  validation.
- Tests: validation + store persistence.

## P4 — Friends & groups (verify Android · build web · visibility endpoint)

**Goal:** working friends/groups on both clients + user-controllable resource visibility
(`private/friends/unlisted_link/public`) — restoring Flutter's sharing model.

1. **Contract verification (first, cheap):** run the Android `SharingScreen` flows against a local
   `apps/api` (two test users): friend request→accept, group create→add member, grant to
   user/group. Fix any DTO drift (paths/shapes were transcribed from Flutter and never exercised).
   Add a request-fidelity test pinning `KtorSharingRepository` to the API contract (mirror the
   offline-manager request-fidelity test pattern).
2. **API — visibility endpoint:** `PUT /api/sharing/resources/{resourceId}/visibility` body
   `{visibility}` → wires to the existing `Resource.ChangeVisibility`, owner-only. Plus GET of a
   single resource's sharing state if not already served by `grants/resources/{id}`. EF/service
   tests alongside the existing Sharing module tests.
3. **Android:** add a visibility selector to the share sheet (track/marker detail → share) using
   the new endpoint; `SharingRepository` gains `setVisibility`. The friends/groups screens already
   exist (`SharingScreen`, `SharingGraphViewModel`) — polish empty/error states once live data
   flows.
4. **Web:** `api/sharing.ts` gains friendships/groups/lookup/setVisibility fetchers (mirror
   Android's paths); a "Friends & sharing" panel (friend code + lookup-by-code add, pending
   requests accept/decline, groups CRUD) — new panel slot beside `AccountSettingsPanel`; sharing
   controls (visibility + grant-to-friend/group) in the track/marker detail panels where the
   share-link button lives today.
- Tests: web vitest for fetchers/panel state; api integration tests for the new endpoint.

---

## Explicitly out of scope (tracked, not planned here)

- **Protected-area search/reverse-geocode** — place-core has the model; needs a data/ingest
  decision (belongs with the shared-ingestion roadmap).
- **Matrikkel/address ingest into the anchors index** — the long-term replacement for P2's direct
  Geonorge calls; already on the places roadmap.
- **WMS custom sources** — engine capability first.
- **Per-marker colour on the server** (web parity for Android's `colorArgb`) — needs a Geo module
  EF migration; adjacent but not one of the seven.
- **Custom photo as location icon** (Flutter had it) — cut for value/weight.
- **Tube dash patterns in the engine** — P0.2 Phase B decision.

## Suggested execution order

P0.2 Android track styling (fixes the latent sync-drop bug) → P0.1 marker export → P0.3 location
marker → P1 elevation backfill → P4 friends/groups verification + visibility (backend contact
early, informs web panel) → P2 search breadth → P3 custom tile URLs → P4 web panel.
