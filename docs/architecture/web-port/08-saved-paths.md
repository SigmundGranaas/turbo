# 08 — Saved paths / tracks

> Browse, inspect, import, export, rename, map, delete, and follow every saved
> track/route/drawn line the signed-in user owns, with an elevation profile.

## Status
- **Android (gold standard):** A **Paths** screen lists all saved paths
  (`PathsScreen.kt` / `PathsViewModel.kt`). Each path is a `SavedPath { id, name,
  path: GeoPath, activityKind?, plannedRoute?, phaseSplits }` where `GeoPath {
  points, source, elevations?, distanceM, ascentM?, descentM?, movingTimeSeconds?,
  recordedAtEpochMs? }`. List supports search (substring on name) and sort
  (Newest / Name / Longest via `FilterChip`s; `sortAndFilterPaths()`). Source is
  the `GeoPathSource` enum `{ Route, Recording, Measure, Saved, Trail, Activity }`
  surfaced as a per-card text label ("Recorded", "Drawn", "Route", "Trail", …).
  Detail screen (`PathDetailScreen`) shows a normalized polyline sketch hero,
  three stat tiles (Distance / Total ascent / Time = moving time), an
  **ElevationCard** with a `Canvas` filled-area chart (x = point index, y =
  elevation normalized, null altitudes bridged; only shown if ≥2 non-null
  elevations) plus ascent/descent summary, an optional CheckpointSplitsCard, and
  actions: **Show on map**, export menu (GPX / GeoJSON / KML), **rename**,
  **delete** (confirm dialog), **share link**. Import is a toolbar action →
  `OpenDocument` → `TrackImport.parse()` (auto-detects GeoJSON `{`, GPX `<gpx`,
  KML `<kml`); name comes from the file's `<name>`/`properties.name`, else the
  file display name minus extension. Export = `Gpx.serialize()` written to
  cache + OS share sheet. "Show on map" draws the track, frames the camera
  (`frameTo`), opens a map selection with a **Follow** action; "Follow this
  track" runs `RouteViewModel.followTrack(...)` (no solver) → doc 11.
- **Web today:** Not implemented. No tracks list, no detail, no import/export, no
  on-map track overlay.
- **Renderer/back-end prerequisites:**
  - `GET/POST/PUT/DELETE /api/tracks/tracks` (auth, cookie). Delta-sync
    `?since=&limit=≤500` → `{ items, deleted:[{id,deletedAt,version}], nextCursor,
    serverTime }`; ETag = `version`, `If-Match` on PUT/DELETE (412 returns
    `{ currentVersion, current }`).
  - Track wire DTO: `geometry { points:[{longitude,latitude}], elevations?:double[] }`
    (`elevations.length == points.length` when present), `metadata { name,
    description?, colorHex?, iconKey?, lineStyleKey?, smoothing }`, `stats {
    distanceMeters, ascentMeters?, descentMeters?, movingTimeSeconds?, recordedAt? }`,
    plus `id, createdAt, updatedAt, version, deletedAt?`.
  - **Backend has no `source` / `activityKind` field** — these are client-only.
    The web app must keep them in a local store keyed by track id (they will not
    round-trip through sync), exactly as Android does (`TrackSync` hardcodes
    `source = "Saved"`, `activityKind = null` on the wire).
  - **Renderer:** the existing `apply_scene` + `ease_to`/`set_camera` is enough to
    draw and frame a track as a `geo-json` line/fill overlay. A **raised tube**
    look requires exposing `set_route_tube` in `turbomap-web` (thin wasm-bindgen
    passthrough; engine already implements it) — optional polish, not required.
  - **Elevation backfill (new):** if a track arrives with no elevations, backfill
    via a `POST /api/tiles/elev/profile` call. **Note:** no Tracks-side
    profile/backfill exists today and there is no live `/api/tiles/elev/profile`
    route yet — see Open questions. Android does **no** backfill (it draws
    whatever elevations were captured at record/import time).

## User stories

### 1. Browse and filter saved tracks
*As a signed-in user, I want a list of all my saved tracks/routes/drawn lines, so that I can find one to inspect or open.*

**Acceptance criteria**
- A **Paths** view lists every non-deleted track owned by the user, hydrated from
  `GET /api/tracks/tracks?since=…` (delta-synced, see doc 18) and cached by
  TanStack Query.
- Each row shows: a source/activity icon, the name, and a meta line
  `"<source> · <distance> · <duration>"`, plus a small inline polyline sketch
  (SVG, normalized from `points`) when the track has >1 point.
- A search box filters by case-insensitive substring on name.
- Sort control offers **Newest** (by `recordedAt` desc, falling back to
  `updatedAt`), **Name** (alpha), **Longest** (by `distanceMeters` desc).
- A source filter (Recorded / Route / Drawn / Trail import / All) filters by the
  client-side `source` value. (Android only labels source; the web port adds the
  explicit filter the prompt calls for.)
- Distance and duration are formatted for the user's units (doc 19): metric km/m,
  duration `Xh Ym` / `Y min` / `—` when unknown.

**Web-specific notes**
- Source/activityKind are read from a local Zustand/IndexedDB store keyed by
  track id, because the backend does not persist them.

### 2. Empty state and unauthenticated state
*As a new or signed-out user, I want a sensible Paths view, so that I understand what to do next.*

**Acceptance criteria**
- **Empty (signed in, zero tracks):** a friendly empty state with a primary CTA to
  plan a route (doc 09), start a recording (doc 10), or **Import a file**.
- **Unauthenticated:** the synced list is hidden (the endpoint requires auth);
  the user sees a "Sign in to see your saved paths" prompt. **Import is still
  offered** — a locally-imported file can be parsed and shown on the map for the
  current session (a transient, not-synced view-only track) without an account.
  Local import does not persist to the backend until the user signs in.

**Web-specific notes**
- The transient local track lives only in memory (no offline persistence this
  phase — see Out of scope) and is lost on reload.

### 3. Inspect a track's detail + elevation profile
*As a user, I want to open a track and see its stats and an elevation chart, so that I can understand the effort and terrain.*

**Acceptance criteria**
- A detail panel shows: name (editable, story 6), three stat tiles **Distance /
  Total ascent / Moving time**, a polyline sketch hero, and an **elevation
  profile** chart.
- The elevation chart (React + SVG/canvas) plots height vs distance: x = cumulative
  haversine distance along `points` (improves on Android's index-based x), y =
  elevation min→max. Null/missing samples are bridged. Chart is shown only when
  ≥2 non-null elevations exist (or after backfill, story 3a).
- Ascent/descent summary numbers accompany the chart.
- Hovering/tapping the chart shows a crosshair with distance + elevation at that
  point (desktop hover, touch drag on mobile).
- If `phaseSplits`/checkpoint data exists locally, an optional splits list renders
  (name + `m:ss · distance`).

**Web-specific notes**
- "Points" count is surfaceable in detail (Android omits it); cheap to show.

### 3a. Backfill a missing elevation profile
*As a user opening a track with no recorded elevations (e.g. a Line/Draw or a flat GPX), I want the chart to still appear, so that I can read the terrain.*

**Acceptance criteria**
- When a track has no `elevations` (or all null), the detail view offers/auto-runs
  a backfill: `POST /api/tiles/elev/profile` with the (optionally resampled)
  `points`, returning a height per sample; the chart renders from the result.
- Backfilled elevations are treated as derived (not written back to the track
  geometry on sync unless the user explicitly saves).
- A failed/unsupported backfill degrades gracefully: the chart is hidden and the
  stat tiles still render distance + (unknown) ascent.

**Web-specific notes**
- This endpoint does not exist yet (Open questions). Until it lands, the chart is
  simply hidden for elevation-less tracks, matching Android.

### 4. Import a GPX / GeoJSON file
*As a user, I want to import a GPX or GeoJSON track from my computer/phone, so that I can keep an externally-recorded route.*

**Acceptance criteria**
- An **Import** button opens a file input accepting `.gpx`, `.geojson`/`.json`,
  `.kml`, `.xml` (and `*/*` fallback).
- Parsing happens **in-browser** (mirror of `TrackImport.parse`): detect by first
  non-whitespace char/tag (`{` → GeoJSON, `<gpx` → GPX, `<kml` → KML).
  - GPX: read `trkpt`/`rtept` with `<ele>`; ignore `wpt`; require ≥2 points.
  - GeoJSON: unwrap FeatureCollection→Feature→geometry; accept
    `LineString`/`MultiLineString`; coords `[lng, lat, ele?]`.
  - KML: `<coordinates>` `lng,lat,ele` tuples.
- Name resolution: file's `<name>` / `properties.name` if non-blank, else the
  picked file's name minus extension, else "Imported track".
- Imported tracks get `source = "Saved"` (trail-import shows under the Trail/Import
  filter per product choice) and are saved to the backend
  (`POST /api/tracks/tracks`) when signed in, or shown transiently when not.
- A malformed/zero-point file shows an inline parse error and imports nothing.

**Web-specific notes**
- No device file-system scanning; one file at a time via the picker (or drag-drop
  onto the list as a nicety).

### 5. Export a track as GPX (and GeoJSON / KML)
*As a user, I want to download or share a track as GPX, so that I can use it in other tools.*

**Acceptance criteria**
- An export menu offers **GPX** (default), GeoJSON, KML.
- The chosen format is generated client-side (mirror `Gpx.serialize`): GPX 1.1
  `<trk><trkseg>` with per-point `<ele>` when present; GeoJSON `Feature/LineString`
  `[lng,lat,ele?]` + `properties.name`; KML 2.2 Placemark.
- On desktop: a Blob + `<a download>` downloads `<safeName>.<ext>` (name sanitized
  to `[A-Za-z0-9-_]`, ≤40 chars).
- On a touch device that supports it, `navigator.share` (Web Share API, file
  share) is offered as an alternative; otherwise it falls back to download.

**Web-specific notes**
- Web Share with files is Safari/Chrome-mobile only; download is the universal
  path.

### 6. Rename a track in place
*As a user, I want to rename a saved track, so that it's easy to recognize.*

**Acceptance criteria**
- A rename control opens a small dialog/inline field prefilled with the current
  name; the value is trimmed.
- Save issues `PUT /api/tracks/tracks/{id}` with `If-Match: <version>` updating
  only `metadata.name` (geometry/stats unchanged). On 412, refetch and retry/merge.
- The list + detail update optimistically and reconcile on response; TanStack Query
  cache for the track + the list is invalidated/updated.

### 7. Show a track on the map
*As a user, I want to view a saved track on the map, so that I can see where it goes and start following it.*

**Acceptance criteria**
- "Show on map" composes a `geo-json` line overlay from the track `points` (and a
  symbol/circle for endpoints), calls `apply_scene`, and frames the camera to the
  track bounds via `ease_to` (eased) with the bottom-sheet inset applied
  (`set_viewport_inset`).
- A map selection/sheet shows the name and a meta subtitle (`"<dist> · ↑ <ascent>"`)
  with a **Follow** action.
- The overlay is removed when the sheet/selection closes (track is drawn only while
  shown), matching Android.
- Optional raised-tube rendering once `set_route_tube` is exposed; flat line until
  then.

**Web-specific notes**
- Bounds framing uses the track's lat/lng extent; `ease_to` mirrors Android's
  `frameTo`.

### 8. Delete a track with confirmation
*As a user, I want to delete a track behind a confirm, so that I don't lose data by accident.*

**Acceptance criteria**
- Delete opens a confirm dialog naming the track.
- Confirm issues `DELETE /api/tracks/tracks/{id}` with `If-Match: <version>`
  (synced tracks become server tombstones; a never-synced/local-only track is
  just dropped from local state).
- The row is removed optimistically; on error it is restored with a toast.
- After deleting from detail, the view returns to the list.

### 9. Start following a track
*As a user, I want to follow a saved track turn-by-turn, so that I can navigate it.*

**Acceptance criteria**
- A **Follow** action (in the show-on-map sheet and detail) hands the track
  geometry + stats to the follow/navigation flow (doc 11) **without** running the
  router (it's an existing line).
- Following requires geolocation permission (browser prompt) and runs while the tab
  is open + screen on (background GPS unavailable — see doc 11).

**Web-specific notes**
- Detailed follow UX (progress, ETA, checkpoints, reroute) is specified in doc 11;
  this story only covers the hand-off.

### 10. Large-track performance
*As a user with a long recorded track (tens of thousands of points), I want the list, chart, and map to stay responsive.*

**Acceptance criteria**
- The list sketch and the elevation chart render from a **decimated** copy of the
  points (e.g. Douglas–Peucker or fixed max-sample) so chart paths stay bounded.
- The full-resolution geometry is fed to the renderer overlay (the engine handles
  large line vertex counts); only the React-drawn previews are decimated.
- Sorting/filtering a large list is O(n) in memory on already-fetched data (no
  refetch per keystroke; debounce the search input).
- Parsing a large import is done off the main thread where feasible (Web Worker) or
  chunked to avoid jank.

## Primary flows (web)

**Browse → detail → follow (happy path):** open Paths → list loads from cached
delta-sync → search/sort/filter → tap a row → detail panel slides up (bottom sheet
on mobile, side panel on desktop) → elevation chart + stats render → "Show on map"
draws + frames the track → tap **Follow** → hands to doc 11.

**Import (signed in):** Import → pick `route.gpx` → parsed in-browser → name from
file → preview drawn + framed → confirm → `POST /api/tracks/tracks` → appears in
list (source = Trail/Import).

**Import (signed out):** Import → parsed → shown transiently on map for the session
→ "Sign in to save" CTA.

**Export:** detail → export menu → GPX → Blob downloaded.

**Edge cases:** parse failure → inline error, nothing imported; rename 412 →
refetch + retry; delete error → row restored + toast; elevation-less track →
backfill attempt or hidden chart; empty list → empty state; unauthenticated → sign-in
prompt + local import only.

## UI / UX on web
- **Paths** is a primary destination in the left rail / nav. On desktop it's a left
  **side panel** (list) with detail in a wider panel or modal; on mobile/touch it's
  a full-height **bottom sheet** list with detail as a stacked sheet.
- When a track is shown on the map, the bottom sheet's height is reported to the
  engine via `set_viewport_inset` so framing/centering accounts for the obscured
  area.
- Search + sort chips + source-filter chips sit at the top of the list (sticky).
- Detail: hero sketch → stat tiles → elevation chart (interactive crosshair) →
  optional splits → action row (Show on map / Follow / export / rename / delete /
  share link).
- Desktop uses pointer hover for chart scrubbing; touch uses drag.

## Data & APIs
- `GET /api/tracks/tracks?since=&limit=≤500` (auth) — delta sync, list source of
  truth. `{ items, deleted, nextCursor, serverTime }`.
- `GET /api/tracks/tracks/{id}` (auth) — ETag = version.
- `POST /api/tracks/tracks` (auth, 201) — create (import, save).
- `PUT /api/tracks/tracks/{id}` (auth, `If-Match`) — rename / geometry replace; 412
  → `{ currentVersion, current }`.
- `DELETE /api/tracks/tracks/{id}` (auth, `If-Match`, 204).
- `POST /api/tiles/elev/profile` (public) — elevation backfill (does not exist yet;
  Open questions).
- Track DTO: `geometry { points:[{longitude,latitude}], elevations?:double[] }`,
  `metadata { name, description?, colorHex?, iconKey?, lineStyleKey?, smoothing }`,
  `stats { distanceMeters, ascentMeters?, descentMeters?, movingTimeSeconds?,
  recordedAt? }`, `id, createdAt, updatedAt, version, deletedAt?`.
- Share link: see doc 16 (`/api/sharing/...` `createLink(remoteId)`) — requires the
  track to be synced (have a server id) and the user signed in.
- **TanStack Query keys:** `['tracks','list']` (delta-synced collection),
  `['tracks', id]` (single). Mutations: create/rename/delete invalidate
  `['tracks','list']`.
- **Zustand slices:** `tracks` UI slice (selectedTrackId, sort, sourceFilter,
  searchText, displayedTrackId for the map overlay) + a `localTrackMeta` map
  (client-only source/activityKind keyed by id) + transient `importedLocalTrack`
  for unauthenticated import.

## Renderer integration
- **Scene:** add a `geo-json` source (inline track geometry) + a `line` layer for
  the path and a `symbol`/`circle` layer for start/end markers; apply via
  `apply_scene`. Remove the source/layers when the track is dismissed.
- **Camera:** `ease_to` to the track bounds (computed from points) with
  `set_viewport_inset` set to the sheet height.
- **Optional tube:** `set_route_tube` for a raised 3D tube — **requires exposing
  `set_route_tube` in `turbomap-web`** (thin wasm-bindgen passthrough; engine
  already implements it). Flat `line` layer is the baseline.
- Coordinate order: track `points` are `{longitude, latitude}` objects on the wire;
  GeoJSON overlay coords are `[lng, lat]` — convert at the boundary.

## Out of scope (this phase)
- Offline persistence of tracks / imported files, "download along route" tile
  download, and crash-recovery drafts (later offline/PWA phase).
- Server-side persistence of `source` / `activityKind` (backend has no such fields;
  kept client-only).
- Drag-to-reorder of multi-segment tracks (tracks are single polylines here).

## Open questions
- Does/should `POST /api/tiles/elev/profile` exist for elevation backfill of
  tracks? It is referenced by the prompt but not implemented today. If not, the
  elevation chart is simply hidden for elevation-less tracks (Android behavior).
- Should the "Trail import" filter map to a distinct client source value, or do
  all file imports stay `source = "Saved"` (Android) and the filter just keys off
  import provenance stored locally?
- Should backfilled elevations be persisted back into the track geometry on next
  edit, or always treated as derived/view-only?
- Confirm units/formatting come from the shared settings store (doc 19).
