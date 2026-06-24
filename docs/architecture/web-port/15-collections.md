# 15 — Collections

> Let users group markers and tracks into named, colour-coded folders that sync
> across devices.

## Status
- **Android (gold standard):** Collections are named folders grouping **markers and tracks**.
  - Domain model `MapCollection { id, name, colorArgb: Long?, icon: String?, itemCount: Int }`. Membership is a separate join: `CollectionItemEntity { collectionId, itemId, itemType }` where `itemType` is the enum `CollectionItemType { Marker, Path }`.
  - Operations (`CollectionsViewModel` / `CollectionRepository`): **create** (`upsert(id=null, name, colorArgb)` → new id `c-<uuid>`), **rename** + **recolor** (`upsert` with existing id), **delete** (soft-delete/tombstone if synced, hard-delete if local-only; confirmed via `ConfirmDeleteDialog`), **addItem(collectionId, itemId, type)** / **removeItem(...)**.
  - **Collections screen** (`CollectionsScreen`): a list of rows, each showing a 40 dp colour badge (folder icon tinted with the collection colour), the name, the item count, and a delete button; a FAB creates a new collection; `CollectionEditorDialog` edits name + colour from 6 preset swatches (`0xFF8F4C38, 0xFF1A73E8, 0xFF2E7D32, 0xFFE0432B, 0xFF6A4FB3`, + null/default); an empty state shows a folder icon + copy.
  - **Add-to-collection** is triggered from marker/track detail → a `CollectionPickerSheet` lists all collections with checkmarks for current membership, allows inline creation, and toggles membership (markers → `Marker`, tracks → `Path`).
  - **Sync** (`CollectionSync`): metadata via `PUT …/{id}` with `If-Match: "<version>"` optimistic concurrency; membership via separate idempotent `POST …/{id}/items` and `DELETE …/{id}/items/{type}/{uuid}`; wire colour is `#RRGGBB` (alpha dropped from the local `Long` ARGB); wire item type is `location` (markers) / `track` (paths).
  - Collections are **organizational only** — there is no dedicated map overlay for a collection; members are still rendered as individual markers/tracks.
- **Web today:** Not implemented. No collections UI, store, or API client exists.
- **Renderer/back-end prerequisites:** `/api/collections/Collections/*` (auth required, cookie-based on web). No renderer changes required for the core feature; an *optional* "highlight a collection on the map" story composes existing geo-json overlay layers (no new passthrough).

## User stories

### 1. Create a collection
*As an organized user, I want to create a named, optionally colour-coded collection, so that I can group related places and tracks.*

**Acceptance criteria**
- A "New collection" action opens an editor with a name field (required) and a colour picker (6 presets + a default/no-colour option, matching Android).
- Saving creates the collection on the backend (`POST`) and shows it immediately in the Collections list with the chosen colour badge and item count 0.
- An empty name is rejected with inline validation; save is disabled until a name is present.
- Colour is sent as `#RRGGBB` (no alpha); default/null sends no colour and renders with the app primary colour.

**Web-specific notes:** requires auth (story acceptance below); the editor is a dialog (desktop) or bottom sheet (touch).

### 2. Rename / recolor a collection
*As a user, I want to edit a collection's name and colour, so that I can keep it tidy as my needs change.*

**Acceptance criteria**
- Editing reopens the same editor pre-filled; saving sends `PUT …/{id}` with `If-Match: "<version>"`.
- The list updates optimistically; a 412 (version conflict) refetches the server copy and reconciles.
- Only changed fields need to be sent (`UpdateCollectionRequest` fields are nullable / "update if non-null").

### 3. Delete a collection
*As a user, I want to delete a collection (with confirmation), so that I can remove ones I no longer need without losing the underlying markers/tracks.*

**Acceptance criteria**
- Delete shows a confirmation dialog before acting.
- On confirm, sends `DELETE …/{id}` with `If-Match`; the row disappears from the list.
- Deleting a collection does **not** delete its member markers/tracks — only the grouping.
- A failed delete (conflict/network) restores the row and surfaces a retry.

### 4. Add / remove items
*As a user, I want to add a marker or track to one or more collections (and remove it), so that I can organize my content from where I see it.*

**Acceptance criteria**
- Marker detail ([06 — Markers](06-markers.md)) and track detail ([08 — Saved paths](08-saved-paths.md)) expose an "Add to collection" action opening a picker.
- The picker lists all collections with a checked state for current membership and supports inline "create new + add".
- Toggling on calls `POST …/{id}/items` with `{ type, uuid }` (`type` = `location` for markers, `track` for tracks; `uuid` = the item's remote id); toggling off calls `DELETE …/{id}/items/{type}/{uuid}`.
- Adds are idempotent server-side (re-adding an existing member is a no-op success).
- Item counts on the Collections list update after membership changes.

**Web-specific notes:** membership uses the item's **remote id** (`uuid`), so the item must be synced first. For a freshly-created local-only marker/track, ensure it has a server id before enabling the add action (cross-link [18 — Sync](18-sync.md)).

### 5. Browse a collection
*As a user, I want to tap a collection and see its members, so that I can review and navigate to grouped content.*

**Acceptance criteria**
- Tapping a collection opens its members list, split (or labelled) by type (markers vs tracks).
- Each member links to its detail and can be located on the map.
- A member can be removed from the collection inline (story 4 remove path).
- An empty collection shows an empty state, not a blank screen.

### 6. (Optional) Show a collection on the map
*As a user, I want to highlight just one collection's items on the map, so that I can focus on a themed subset.*

**Acceptance criteria**
- An optional "Show on map" toggle filters the rendered markers/tracks to that collection's members (or emphasises them, e.g. tinted with the collection colour).
- Toggling off restores the default rendering.

**Web-specific notes:** composes existing `geo-json` sources + line/circle/symbol layers (see [04 — Vector overlays](04-vector-overlays.md) / [05 — Location & entities](05-location-and-entities.md)); **no new `turbomap-web` passthrough required**. This is optional polish, not parity-critical (Android has no such overlay).

## Primary flows (web)

**Happy path — create + populate:** open Collections panel → "New collection" → name + colour → save (`POST`) → open a marker's detail → "Add to collection" → pick the new collection (`POST …/{id}/items`) → item count increments.

**Browse + remove:** open Collections → tap a collection → members list → remove an item (`DELETE …/{id}/items/{type}/{uuid}`) → list + count update.

**Empty state:** a user with no collections sees a folder icon + explanatory copy + a primary "Create collection" button. A collection with no members shows an empty members state.

**Unauthenticated:** Collections require login. An anonymous user sees a sign-in prompt where the Collections panel would be (cross-link [17 — Account & auth](17-account-auth.md)); the "Add to collection" action on marker/track detail is hidden or prompts sign-in.

**Network error / conflict:** create/edit/delete failures roll back the optimistic UI and surface a retry. A `412` on edit/delete refetches the server copy and re-applies the user's change against the new version.

## UI / UX on web

- **Collections panel/screen** in the app shell side panel (desktop) or a full sheet (touch): a list of rows — colour badge + name + item count + overflow (rename/recolor/delete). A primary "New collection" button / FAB.
- **Editor** dialog (desktop) / bottom sheet (touch): name field + 6 preset colour swatches + default.
- **Picker** (add-to-collection): a compact list of collections with checkboxes + inline create, opened from marker/track detail.
- **Member view:** reuses marker/track list rows, grouped by type.
- When a sheet is open over the map on touch, apply `set_viewport_inset` so the map centre isn't occluded.
- Confirmation dialog for delete (matches Android).

## Data & APIs

Base controller route: **`/api/collections/Collections`** (auth required; cookie-based on web via `apiFetch` `credentials: 'include'`).

| Op | Method + path | Body / params | Notes |
|---|---|---|---|
| List / delta-sync | `GET /api/collections/Collections?since=&limit=` | `since` (ISO datetime), `limit` (≤ 500) | Returns `CollectionsDeltaResponse { items, deleted[], serverTime, nextCursor }` |
| Get one | `GET /api/collections/Collections/{id}` | — | Returns `CollectionResponse` + ETag |
| Create | `POST /api/collections/Collections` | `CreateCollectionRequest` | 201 → `CollectionResponse` |
| Update | `PUT /api/collections/Collections/{id}` | `UpdateCollectionRequest` + `If-Match` | Optimistic concurrency |
| Delete | `DELETE /api/collections/Collections/{id}` | `If-Match` | 204 |
| Add item | `POST /api/collections/Collections/{id}/items` | `{ type, uuid }` | Idempotent; 204 |
| Remove item | `DELETE /api/collections/Collections/{id}/items/{type}/{itemUuid}` | — | 204 |

**Shapes (key fields):**
- `CreateCollectionRequest { name, description?, colorHex?, iconKey?, sortOrder?, savedFilter? }`.
- `UpdateCollectionRequest { name?, description?, colorHex?, iconKey?, sortOrder?, savedFilter?, clearSavedFilter }` (non-null = update).
- `CollectionResponse { id, name, description?, colorHex?, iconKey?, sortOrder, savedFilter?, items: ItemRef[], createdAt?, updatedAt?, version }`.
- `ItemRef { type: "location"|"track", uuid }`.
- `CollectionsDeltaResponse { items: CollectionResponse[], deleted: { id, deletedAt, version }[], nextCursor?, serverTime }`.

**Auth:** required (bearer/cookie). **Delta-sync confirmed** — `GET` supports `?since=` + tombstones (mirrors markers/tracks); fold into the cloud sync loop ([18 — Sync](18-sync.md)).

**TanStack Query keys:** `['collections']` (list/delta), `['collection', id]` (detail + members). Mutations (create/update/delete/add-item/remove-item) invalidate `['collections']` and the affected `['collection', id]`, with optimistic updates + `If-Match` from the cached `version`.

**Zustand:** a small `collectionsUi` slice for panel/editor/picker open state and the currently-browsed collection id.

**Web mapping notes:**
- Colour is `colorHex` `#RRGGBB` on the wire (no alpha) — the web client uses hex directly; no need for the Android `Long` ARGB conversion.
- Wire item `type` is `location` (markers) / `track` (paths) — map web "marker" → `location`, web "track" → `track`.
- `savedFilter` (smart-collection JSON) and `description`/`iconKey`/`sortOrder` exist on the backend but are not part of the core Android UX; web can pass them through and surface later.

## Renderer integration

- Core feature: **none** — collections are panel data, not a map layer.
- Optional story 6 ("Show on map"): compose existing `geo-json` sources + circle/line/symbol layers filtered to the collection's members; reuses the entity-rendering path from [05 — Location & entities](05-location-and-entities.md). No `turbomap-web` passthrough required.

## Out of scope (this phase)
- Offline-first collection persistence + offline membership edits (offline phase; web relies on the delta-sync loop while online).
- Smart/saved-filter collections (`savedFilter`) UI — backend field exists; defer the UI.
- Sharing a collection with friends/groups (see [16 — Sharing & social](16-sharing-social.md)).

## Open questions
- Confirm collections join the unified web **delta-sync** loop ([18 — Sync](18-sync.md)) vs. plain on-demand fetch-on-open. (Backend supports `?since=`; recommend folding into sync.)
- Should the optional "Show on map" highlight (story 6) ship in this phase or defer? Android has no equivalent, so it's web-only polish.
- Should `description` / `iconKey` / `sortOrder` be surfaced in the web editor, or kept as pass-through to preserve data set by other clients?
