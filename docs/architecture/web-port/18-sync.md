# 18 — Cloud sync

> Make a signed-in user's markers, tracks, and collections appear and stay consistent across devices — on the web this is online-first live API reads/writes (server is the source of truth), with delta endpoints, optimistic-concurrency mutations, conflict handling, and a sync-status indicator. Full offline-queue sync is deferred.

## Status
- **Android (gold standard):** `SyncEngine` (implements `SyncController`) orchestrates per-domain syncers (`MarkerSyncer` "geo", `TrackSyncer` "tracks", `CollectionSyncer` "collections", `SharedSyncer` "shared") when **signed-in AND `cloudSyncEnabled`**. Each domain: **pull** deltas via `?since=` cursor → **merge** (server wins unless a local row is dirty+newer; tombstones delete unless locally revived) → **push** dirty rows (POST create / PUT+`If-Match` update / DELETE+`If-Match`) → **persist** the new cursor (`serverTime`) in DataStore (`turbo_sync_cursors`). 412 on a conditional write = conflict (server copy in body). A mutex serializes syncs; cursors clear on sign-out; **Sync now** triggers manually; `SyncStatus` = Idle / Syncing / Failed(domains).
- **Web today:** **not implemented as sync.** TanStack Query is already set up (`QueryClientProvider`) and is the cache. There is no local DB to reconcile.
- **Renderer/back-end prerequisites:**
  - **No renderer change.** Synced entities feed the existing `geo-json`/overlay layers (docs 05/06/08/15).
  - **Requires auth (doc 17).** All data endpoints require the session cookie.
  - Endpoints: `GET /api/geo/locations?since=&limit=`, `GET /api/tracks/Tracks?since=&limit=`, `GET /api/collections/Collections?since=&limit=` (each → `{ items[], deleted[], serverTime, nextCursor }`); creates `POST` → `201` + `ETag: version`; updates `PUT /…/{id}` + `If-Match`; deletes `DELETE /…/{id}` + `If-Match`. Shared: `GET /api/sharing/resources/sync?since=&types=` (doc 16).

## The web sync model (be explicit)
The web is **online-first**: there is **no local database to reconcile this phase**, so "sync" is mostly just **live API reads/writes through TanStack Query**, not a background pull/merge/push engine like Android.

- **Server is the source of truth.** Reads come straight from the API; the Query cache is a short-lived view, not durable storage.
- **Delta endpoints are still used** so list refetches are cheap: queries pass `?since=<last serverTime>` and apply `items`/`deleted` to the cached list, then store the new `serverTime` as the cursor (in-memory / `localStorage` per query, not a DB).
- **Mutations write through** immediately: `POST` create, `PUT`+`If-Match: version` update, `DELETE`+`If-Match: version` delete. After a mutation, the relevant Query key is invalidated (or the cache is patched optimistically and reconciled).
- **Background refetch on window focus** keeps multiple open tabs/devices roughly live (no push channel this phase).
- **Conflicts** surface as a 412/409 on a conditional write → refetch the server copy and either auto-merge or notify the user (story 3).
- **Offline:** because there is no write queue this phase, a write that fails on network error **fails with a retry affordance** rather than silently queuing — stated honestly below and in Open questions.

## User stories

### 1. My data appears on all my devices
*As a signed-in user, I want the markers/tracks/collections I created elsewhere to show up here, so that my data follows me.*

**Acceptance criteria**
- On sign-in (and on first load of a list view), the web fetches the full list (no `since=`) for that domain and renders it; subsequent refetches pass `?since=<cursor>` and apply only `items`/`deleted`.
- Lists reflect the server within one refetch of opening the view or regaining focus.
- Sign-out clears all account-scoped Query caches and cursors so the next user/visitor starts clean (mirrors Android clearing cursors).

**Web-specific notes**
- Cursors are stored per domain (e.g. `localStorage['turbo.sync.cursor.tracks']`) so a focus-refetch can be a cheap delta even across reloads; a missing/!valid cursor falls back to a full fetch.

### 2. My changes propagate
*As a user editing on the web, I want my create/update/delete to reach the server and be visible elsewhere, so that edits aren't lost.*

**Acceptance criteria**
- Create → `POST`; the response `ETag`/version + id are stored so subsequent edits carry `If-Match`.
- Update → `PUT /…/{id}` with `If-Match: "<version>"`; on success the cache is updated with the new version from the response/ETag.
- Delete → `DELETE /…/{id}` with `If-Match: "<version>"`.
- After any mutation the list/detail Query keys are invalidated so the UI shows the committed state; optimistic UI is allowed but rolls back on error.
- Another open tab/device sees the change on its next focus-refetch.

**Web-specific notes**
- Without `If-Match` the server can't detect concurrent edits, so the web must always send the last-known version on PUT/DELETE.

### 3. Conflicts are handled gracefully
*As a user, I want a clear outcome when my edit collides with a newer server version, so that I don't silently overwrite or lose data.*

**Acceptance criteria**
- A conditional write returning **412 Precondition Failed** (or 409) is treated as a conflict, not a generic error.
- On conflict the web refetches the server copy, then either: (a) auto-merges if the change is non-overlapping and re-submits with the new version, or (b) notifies the user ("This item changed elsewhere — your edit wasn't saved; review the latest") and shows the server version, preserving the user's pending edit so they can re-apply.
- A **404** on update/delete (resource deleted server-side) is surfaced as "This item was deleted elsewhere" and removed from the local cache.
- No conflict path overwrites the server blindly (no retry with a stale `If-Match`).

**Web-specific notes**
- Matches Android's `SyncDecisions` philosophy (don't clobber newer data) but, lacking a local dirty/merge store, the web's default is **notify + show latest + keep the user's pending edit in the form**, not silent merge.

### 4. See sync status and sync manually
*As a user, I want to see whether my data is up to date and force a refresh, so that I trust what I'm looking at.*

**Acceptance criteria**
- A sync-status indicator (on the account screen, doc 17, and optionally a subtle shell badge) reflects: **Live / up to date** (queries fresh), **Refreshing** (a refetch is in flight), and **Error** (last refetch/mutation failed, with retry).
- A **Sync now** button invalidates/refetches all account-scoped Query keys (markers, tracks, collections, shared) — the web equivalent of Android `syncNow()`.
- When **cloud sync is disabled** in settings (doc 19), the indicator shows "Sync off" and Sync now is disabled; account-scoped reads/writes still work as direct API calls but the status copy reflects the toggle. *(Confirm in Open questions whether the web honors the toggle the same way Android gates the engine.)*
- The indicator never blocks the UI; it's informational.

**Web-specific notes**
- "Status" on the web is derived from TanStack Query state (`isFetching`, `isError`, `dataUpdatedAt`) across the account-scoped queries — not a long-running engine state machine.

## Primary flows (web)

**Happy path — open a list signed-in**
1. Sign-in resolves (`['session']`).
2. List view mounts → `GET /api/tracks/Tracks` (full) → render → store `serverTime` cursor.
3. On focus later → `GET /api/tracks/Tracks?since=<cursor>` → apply `items`+`deleted` → update cursor.

**Happy path — edit propagates**
1. User edits a track → `PUT /api/tracks/Tracks/{id}` with `If-Match: "<version>"`.
2. 200 → cache patched with new version → list invalidated.
3. Another device focus-refetches and sees it.

**Edge — concurrent edit conflict**
- `PUT` returns 412 → refetch server copy → notify "changed elsewhere", show latest, keep pending edit → user re-applies on the new version.

**Edge — deleted elsewhere**
- `PUT`/`DELETE` returns 404 → remove from cache, toast "deleted elsewhere".

**Edge — offline / network error**
- A read failure shows the indicator Error state + retry; the last cached list stays visible.
- A write failure shows a per-action error with **Retry** and keeps the user's edit in the form — **it does not queue** for later (deferred; see Out of scope / Open questions).

**Edge — sync disabled / signed-out**
- Sync off → indicator "Sync off". Signed-out → account-scoped lists are empty/local-only and the indicator is hidden (matches doc 17 story 7).

## UI / UX on web
- **Indicator:** a small status chip in the app shell (e.g. near the account avatar) and a fuller status row on the account screen with **Sync now**.
- **Conflict notice:** a non-blocking banner/toast with "Review latest" that opens the refreshed item; the edit form retains pending values.
- **Composition with canvas:** none directly; synced entities render via their feature docs. Sync UI is shell chrome only.
- **Responsive:** chip collapses to an icon on narrow viewports; account screen shows the full status + button.

## Data & APIs
- **Markers:** `GET /api/geo/locations?since=&limit=500` → `{ items, deleted, serverTime, nextCursor }`; `POST` (201 + ETag); `PUT /…/{id}` + `If-Match`; `DELETE /…/{id}` + `If-Match`. *(auth)*
- **Tracks:** `GET /api/tracks/Tracks?since=&limit=500` (+ POST/PUT/DELETE as above). *(auth)*
- **Collections:** `GET /api/collections/Collections?since=&limit=500` (+ POST/PUT/DELETE; items via `POST /…/{id}/items`, `DELETE /…/{id}/items/{type}/{uuid}`). *(auth)*
- **Shared:** `GET /api/sharing/resources/sync?since=&types=collection,path,location` → `{ items: envelope[], serverTime }` (read-only adoption; doc 16). *(auth)*
- **Response shapes:** entity DTOs carry `id`, `version` (for `If-Match`), `updatedAt`/`createdAt`; tombstones in `deleted[]` carry `{ id, deletedAt, version }`; cursor = `serverTime` (ISO-8601).
- **State:** TanStack Query keys per domain (`['markers']`, `['tracks']`, `['collections']`, `['shared','resources']`); mutations invalidate their key; cursors persisted per-domain in `localStorage`. `Sync now` = invalidate-all account-scoped keys. Status derived from Query flags.
- **External:** none.

## Renderer integration
- **None new.** Synced entities are rendered by their feature docs (markers doc 06, tracks doc 08, collections doc 15) via `geo-json` sources + layers and `apply_scene`. Sync only changes *when* that data arrives, not how it's drawn.

## Out of scope (this phase)
- **Offline write queue / durable local DB** and full bidirectional reconcile (Android's dirty-row push). The web is online-first; offline writes fail-with-retry, not queue. Deferred to the offline/PWA phase.
- Background sync while the tab is closed / Service Worker periodic sync.
- Real-time push (WebSocket/SSE) for cross-device live updates — this phase relies on focus-refetch.
- Cross-tab cache coordination (BroadcastChannel) — focus-refetch is sufficient for now.

## Open questions
- **Offline writes:** confirm "fail with retry, no queue" is the accepted decision this phase (vs. a minimal optimistic queue). Documented here as fail-with-retry.
- **Sync toggle semantics on web:** does disabling cloud sync (doc 19) actually stop account-scoped reads/writes, or just background refetch? On Android it gates the whole engine; the web has no engine, so define what "off" means (proposal: stop background focus-refetch + Sync now, but allow explicit user reads/writes).
- **Cursor durability:** is `localStorage`-persisted `serverTime` per domain acceptable, or should the web always full-fetch on boot for simplicity? (Proposal: persist + delta on focus, full-fetch on cursor miss.)
- **Conflict default:** confirm "notify + show latest + keep pending edit" over any attempt at automatic field-merge for the first iteration.
- **412 vs 409:** confirm which status the conditional writes return (Android references 412); handle both as conflict.
