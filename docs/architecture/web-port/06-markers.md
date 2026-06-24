# 06 — Markers / POIs

> Let a signed-in user drop, edit, inspect, and delete named map markers ("locations") with notes, an activity kind, a custom colour, attached photos, and live weather at the point.

## Status
- **Android (gold standard):** Long-press anywhere on the map drops a marker. A name sheet opens pre-filled with a reverse-geocoded place name (e.g. "Near Stetind", "In Rago nasjonalpark"), with an activity-kind icon grid (18 kinds, default `Cabin`), a 7-swatch colour palette (`null`/Auto + 6 ARGB presets), and a notes field. Markers are color-coded pins. Long-press a marker / tap **Delete** in its detail sheet → confirmation dialog. The detail sheet (a modal bottom sheet rendered by the generic `MapEntityDetailHost`) shows the kind icon + name, a subtitle (reverse-geocoded description), a body slot with the photo grid + live weather at the marker, and an action bar (Edit / Delete / Add photo / Share). Markers list is reachable from search. CRUD + delta-sync go to `/api/geo/locations` with `If-Match` optimistic concurrency. Source: `feature/map/.../markers/MarkerSheets.kt`, `core/map/.../MapEntityDetailHost.kt`, `core/sync/.../MarkerSync.kt`, `core/model/.../domain/Models.kt`.
- **Web today:** Not implemented. No marker layer, no create/edit/detail UI, no `/api/geo/locations` client. The web baseline is basemap + 2D pan/zoom + auth plumbing only.
- **Renderer/back-end prerequisites:**
  - Markers render as a `geo-json` source + `circle`/`symbol` layers composed into the Scene and pushed via `apply_scene` (see `05-location-and-entities.md` for the overlay compositor pattern). No new engine feature required for rendering.
  - **Hit-testing** a tapped marker: either `unproject(x,y)` + nearest-feature search in JS against the marker set, **or** the engine's `hit_test` once it is exposed in `turbomap-web` (**requires exposing `hit_test` in `turbomap-web`** — a thin wasm-bindgen passthrough over the existing engine method). Start with `unproject`; adopt `hit_test` if pixel precision is insufficient.
  - Endpoints: `GET /api/places/reverse?lat=&lon=` (public), `/api/geo/locations` CRUD + `?since=` delta-sync (**auth required**, cookie-based).

## User stories

### 1. Drop a marker by long-pressing the map
*As a signed-in user, I want to press-and-hold (or right-click) a point on the map and get a pre-named marker sheet, so that I can save a place in one gesture without typing its name.*

**Acceptance criteria**
- A **long-press** opens the marker editor. Long-press is: **press-and-hold ~500 ms on touch**, **OR right-click (contextmenu) on desktop pointer**. Both resolve the pressed pixel → lat/lng via `unproject`.
- On open, the app fires `GET /api/places/reverse?lat=&lon=` and pre-fills the **Name** field with the returned place name once it arrives, **only if the user has not started typing** (do not clobber user input).
- The editor exposes: **Name** (required), **Notes** (optional, multiline), **Activity kind** (icon grid of the 18 kinds — see `14-activities.md`; default `Cabin`), and **Colour** (palette: Auto + 6 presets — see colour model below).
- A provisional pin is shown at the pressed point while the sheet is open (ghost/half-opacity), and is committed to the marker layer on Save.
- **Save** issues `POST /api/geo/locations` with geometry + display (name, description, icon=kind). On 2xx the server `id` + `version` are stored and the pin becomes permanent; the TanStack Query marker cache is invalidated/updated.
- **Cancel** / dismiss removes the provisional pin and fires no write.

**Web-specific notes**
- Right-click must `preventDefault()` to suppress the native browser context menu over the canvas.
- Reverse-geocode is best-effort: if it is slow or errors, the Name field stays empty/placeholder and the user can type. Never block Save on reverse-geocode.

### 2. Edit a marker's name, notes, colour, and kind
*As a signed-in user, I want to change a marker's details after creating it, so that I can correct the name or recategorise the place.*

**Acceptance criteria**
- The detail sheet's **Edit** action opens the same editor with all fields pre-filled (name, notes, kind, colour).
- **Save** issues `PUT /api/geo/locations/{id}` with an `If-Match: "{version}"` header carrying the locally-known version.
- On **412 Precondition Failed** (a concurrent edit bumped the server version), the app refetches the marker, surfaces a "this marker changed elsewhere — review and re-save" notice, and does not silently overwrite.
- A successful edit updates the marker layer paint (colour/kind) and the detail sheet without a full reload.

**Web-specific notes**
- Colour is **local-only on Android today** (the sync wire carries only name/description/icon). See colour model + Open Questions — the web must decide whether to persist colour client-side (localStorage/IndexedDB keyed by marker id) to match Android, or push for a server field.

### 3. Delete a marker
*As a signed-in user, I want to delete a marker I no longer need, so that my map stays uncluttered.*

**Acceptance criteria**
- Delete is reachable two ways: **long-press the marker** → confirm, and the detail sheet's **Delete** action → confirm.
- A confirmation dialog ("Delete <name>?") is required; it is destructively styled and cancellable.
- Confirm issues `DELETE /api/geo/locations/{id}` with `If-Match: "{version}"`. On success the pin is removed from the marker layer and the cache.
- A 412 on delete is handled like edit (refetch + notice), not a silent no-op.

### 4. Inspect a marker (detail sheet)
*As a signed-in user, I want to tap a marker and see its full detail, so that I can read its notes, see its photos, and check conditions there.*

**Acceptance criteria**
- Tapping a marker pin opens its detail sheet (bottom sheet on mobile, side panel on desktop — see UI/UX).
- The sheet shows: **kind icon + name**, a **subtitle** (reverse-geocoded description if stored), **coordinates** (formatted lat/lng, copyable), **notes**, a **photo grid** (see `07-photos.md`), and **live weather at the marker** (see story 5).
- Hit-test resolves the tapped pin deterministically; tapping empty map closes the sheet.
- Opening the sheet insets the map viewport so the marker is not hidden under the sheet (`set_viewport_inset`), matching the location/entity pattern in `05`.

### 5. See live weather at the marker
*As a signed-in user, I want current weather shown in the marker detail, so that I can judge conditions at that exact place.*

**Acceptance criteria**
- The detail sheet shows a compact current-conditions block (temp, symbol, wind) for the marker's lat/lng, fetched lazily when the sheet opens.
- Weather fetching, caching, units, and the full conditions panel are owned by `13-conditions-weather.md`; this doc only embeds the summary and links out. The marker sheet passes its coordinate to the shared conditions query.
- If conditions fail to load, the block shows an inline retry, and the rest of the sheet still renders.

### 6. Browse markers in a list
*As a signed-in user, I want a list of my markers, so that I can find and fly to one without hunting on the map.*

**Acceptance criteria**
- A markers list is reachable from the app shell / search (mirrors Android, where markers surface through search results filtered by kind — see `14-activities.md`).
- Each row shows kind icon, name, and (optionally) distance from current view/location. Tapping a row `ease_to`s the marker and opens its detail.
- **Empty state:** with zero markers, the list shows a friendly prompt explaining how to drop one ("Long-press the map to add a marker").

## Primary flows (web)

**Drop (happy path):** long-press/right-click map → `unproject` → open editor + fire reverse-geocode → name pre-fills → user adjusts kind/colour/notes → **Save** → `POST /api/geo/locations` → store id+version → commit pin → invalidate `['locations']`.

**Edit:** detail sheet → Edit → editor pre-filled → Save → `PUT …/{id}` with `If-Match` → update layer + cache. **412 →** refetch + "changed elsewhere" notice.

**Delete:** long-press pin (or detail → Delete) → confirm dialog → `DELETE …/{id}` with `If-Match` → remove pin + cache entry.

**Inspect:** tap pin → hit-test → open detail (inset viewport) → lazy-load weather + photo grid.

**Unauthenticated:** `/api/geo/locations` requires auth. If a signed-out user long-presses, prompt to sign in. **Decision (see Open Questions):** for this online phase, marker creation requires auth — there is **no local-only anonymous marker store** on web (offline/local persistence is the deferred offline/PWA phase). The long-press editor for a signed-out user shows a sign-in CTA instead of a Save button.

**Network error:** a failed `POST`/`PUT`/`DELETE` keeps the editor open (or restores the pin on delete) with a retry; never drops the user's input. The provisional pin is not committed until the server confirms.

**Empty state:** no markers → no marker layer features; list view shows the "long-press to add" prompt.

## UI / UX on web
- **Editor:** bottom sheet on mobile / centered modal or right side panel on desktop. Name + notes text fields, an icon FlowRow for the 18 kinds, a colour swatch row, Save/Cancel.
- **Detail:** bottom sheet (mobile) / persistent right side panel (desktop ≥ md). The sheet applies `set_viewport_inset` so the active pin stays visible above it.
- **Touch vs pointer:** long-press (hold) on touch; right-click on desktop. A "+" / drop-pin affordance in the map toolbar is an acceptable secondary path on desktop (then click to place), but the long-press/right-click gesture is the primary parity path.
- **Colour & kind** feed the marker layer paint so pins are colour-coded live as the user edits.

## Data & APIs
- **Reverse geocode (public):** `GET /api/places/reverse?lat=&lon=` → place name/description used to pre-fill the editor and as the detail subtitle. (Android cascades raw Kartverket/Geonorge/Miljødirektoratet endpoints; web uses the app's `/api/places/reverse` proxy.)
- **Markers (auth):** `/api/geo/locations`
  - `POST` create → body `{ geometry:{longitude,latitude}, display:{name,description,icon} }` → returns `{ id, geometry, display, createdAt, updatedAt, deletedAt, version }`.
  - `GET /{id}` fetch.
  - `PUT /{id}` update (header `If-Match: "{version}"`).
  - `DELETE /{id}` (header `If-Match: "{version}"`).
  - `GET ?since={ISO8601}&limit=500` delta-sync → `{ items[], deleted[ {id,deletedAt,version} ], serverTime, nextCursor }`. Delta-sync mechanics belong to `18-sync.md`; this doc consumes the resulting marker set.
- **Auth:** cookie-based; `apiFetch` uses `credentials: 'include'`. All `/api/geo/locations` calls require a session.
- **TanStack Query keys:** `['locations']` (list), `['locations', id]` (detail). Mutations (create/update/delete) optimistically update `['locations']` and reconcile on settle.
- **Zustand:** `activeMarkerId` / editor-open / provisional-pin state in the UI store.
- **Colour persistence:** see Open Questions — colour is not on the sync wire today.

## Renderer integration
- **Source:** one `geo-json` source holding all markers as points (id, kind, colour properties).
- **Layers:** a `circle` layer (or `symbol` with a kind glyph sprite) painted by per-feature colour (resolved from the marker's `colorArgb`, falling back to the kind/Auto colour — see `14-activities.md`). Pushed via `apply_scene`.
- **Hit-test:** JS `unproject` + nearest-point search initially; **expose `hit_test` in `turbomap-web`** (thin passthrough) if pixel-accurate picking against the rendered symbol is needed.
- **Viewport inset:** `set_viewport_inset` when the detail sheet/panel is open.
- No raised-geometry passthrough needed (markers are flat overlay features, unlike route tubes).

## Out of scope (this phase)
- Offline / local-only anonymous markers and crash-recovery draft persistence (deferred offline/PWA phase).
- Bulk operations, marker→collection assignment (see `15-collections.md`), sharing a single marker (see `16-sharing-social.md`).
- Activity-specific observation forms (see `14-activities.md`).

## Open questions
1. **Colour sync.** Android stores `colorArgb` **locally only** (sync carries only name/description/icon). Should web (a) persist colour client-side keyed by marker id to match Android exactly, or (b) push for a server colour field so colour survives across devices? Recommend (a) for parity now, (b) as a backend ask.
2. **Hit-testing.** Confirm whether `unproject`-based JS picking is precise enough, or whether `hit_test` must be exposed in `turbomap-web` for this phase.
3. **Default kind.** Android defaults new markers to `Cabin`. Keep this on web, or default to last-used / no kind?
4. **Coordinates display format** (decimal vs DMS) and whether to honour the units/format setting (`19-settings.md`).
