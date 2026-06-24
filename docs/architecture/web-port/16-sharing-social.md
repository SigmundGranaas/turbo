# 16 — Sharing + social

> Let a web user share a synced track/marker/collection via a link, manage a friend graph (via "turbo-XXXX" friend codes) and groups, and open someone else's shared link in a read-only public view.

## Status
- **Android (gold standard):**
  - **Share a synced resource by link.** From a track/marker detail, "Share" creates a link grant (`POST /api/sharing/grants/links`, default role `viewer`) and produces a shareable URL. **Requires signed-in AND the resource already synced** to the account — if not signed in, the app prompts to sign in; if signed in but the resource isn't synced yet, it prompts to enable cloud sync / sync first (a link can only point at a server-side resource id).
  - **Friend code** "turbo-XXXX" shown on the account screen (`SharingViewModel.friendCode`, from `GET /api/sharing/me/profile`), lazily generated on first read, copy-to-clipboard.
  - **Social graph** via `SharingGraphViewModel` + `SharingScreen` (tabs: Friends / Groups / Shared): add a friend by code (`GET /api/sharing/users/lookup?code=` → `POST /api/sharing/friendships/request`), accept/decline (`POST /friendships/accept` / `DELETE /friendships/{otherUserId}`), remove friend, create/rename/delete groups, add/remove group members, and a "Shared" tab listing resources granted to you (`GET /api/sharing/resources/sync?since=`).
  - **Per-resource grant management:** list grants for a resource, grant to a user/group/link, revoke any of them.
- **Web today:** **not implemented.** No sharing UI, no friend code, no social graph, no shared-link view route, no `/api/sharing/*` client.
- **Renderer/back-end prerequisites:**
  - **No renderer change.** A shared track/marker renders with the same `geo-json` source + line/symbol/circle layers (and, for tracks, the `set_route_tube` passthrough from doc 08/09 once exposed) used everywhere else; the public view just feeds the canvas a read-only Scene.
  - Endpoints (all auth except the public link-view path below): `POST /api/sharing/grants/links`, `POST /api/sharing/grants/links/{token}/redeem`, `GET /api/sharing/me/profile`, `GET /api/sharing/users/lookup?code=`, `GET /api/sharing/friendships?status=`, `POST /api/sharing/friendships/request|accept`, `DELETE /api/sharing/friendships/{otherUserId}`, `GET|POST /api/sharing/groups`, `PUT /api/sharing/groups/{id}/name`, `DELETE /api/sharing/groups/{id}`, `POST /api/sharing/groups/{id}/members`, `DELETE /api/sharing/groups/{id}/members/{userId}`, `GET /api/sharing/grants/resources/{resourceId}`, `POST /api/sharing/grants/users|groups`, `DELETE /api/sharing/grants/resources/{resourceId}/{users|groups|links}/{id}`, `GET /api/sharing/resources/sync?since=&types=`.
  - **Requires auth (doc 17) + sync (doc 18) to be live** — sharing depends on a synced resource id and a signed-in session cookie.
  - **Open question (see below):** redeeming a link token publicly (an anonymous recipient who is not signed in) vs. requiring sign-in to redeem. The web public-view route is designed around the answer.

## User stories

### 1. Share a track/marker via link
*As a signed-in user, I want to turn one of my synced tracks or markers into a shareable link, so that I can send it to anyone.*

**Acceptance criteria**
- A "Share" action is available on a track detail (doc 08) and marker detail (doc 06).
- If the user is **not signed in**, the action opens a sign-in prompt (doc 17) instead of creating a link, with copy explaining sharing needs an account.
- If the user is signed in but the resource is **not yet synced** (local-only / sync disabled), the action prompts to enable cloud sync and sync now (doc 18) before a link can be made; it does not create a dangling link.
- For a synced resource, the action calls `POST /api/sharing/grants/links` with `{ resourceId, role: 'viewer' }` and on success returns a `linkToken`; the UI composes the share URL (e.g. `https://<web-origin>/s/{linkToken}`).
- The dialog shows the URL with a one-tap **Copy link** button and confirms with a toast on copy.
- Creating a second share of the same resource reuses/links to the same resource (a new link grant is acceptable); the UI does not error on re-share.

**Web-specific notes**
- The share *URL* points at the **web app's** public route (`/s/{token}`), not at the API; the web route is what renders the read-only view and performs the redeem call.

### 2. Copy a share link to the clipboard
*As a sharer, I want to copy the link with one tap, so that I can paste it into any messaging app.*

**Acceptance criteria**
- Copy uses `navigator.clipboard.writeText`; on success a transient confirmation appears.
- If the Clipboard API is unavailable or blocked (insecure context / permission), the link text is selectable in a focused input as a fallback and the UI says "Copy manually".
- The dialog also offers the native share sheet via `navigator.share({ url })` when available (mobile browsers), falling back to copy on desktop.

**Web-specific notes**
- `navigator.clipboard` requires a secure context (https / localhost) — fine for the deployed app; note the manual-select fallback for any non-secure preview.

### 3. Open someone's shared link (public read-only view)
*As a recipient, I want to open a link I was sent and see the shared track/marker on the map, so that I can view it without an account.*

**Acceptance criteria**
- Visiting `/s/{token}` resolves the token: redeem via `POST /api/sharing/grants/links/{token}/redeem`, which returns `{ resourceId, resourceType, role }`.
- The app then loads the resource (track via `/api/tracks/...`, marker via `/api/geo/...`, or collection) and renders it on the map in a **read-only** shared view: camera fits the resource bounds, the entity is drawn, a detail panel shows name/metadata, and editing/sharing controls are hidden (role `viewer`).
- If the recipient **is signed in**, redeem also materializes a per-user grant so the resource shows up in their "Shared" tab afterward.
- **Link not found / invalid token** → a clear "This link is no longer available" page (404), with a button back to the app.
- **Revoked / expired link** → same friendly "no longer available / expired" page (do not leak resource existence).
- **Whether an anonymous (signed-out) recipient can view** depends on the redeem endpoint's auth (see Open questions): if redeem requires auth, the public route shows a sign-in gate first; if not, it renders directly. The route is built to handle both by checking the redeem response / 401.

**Web-specific notes**
- This is a distinct **route** (`react-router` is already a dependency but unused). It mounts the same `<TurboMapCanvas>` in a stripped, read-only chrome — no rail, no edit affordances.
- The shared view is deep-linkable and SSR-friendly later; for this phase it is a client route that fetches on mount.

### 4. See and add friends by friend code
*As a user, I want to see my own "turbo-XXXX" code and add friends by theirs, so that I can build a friend graph.*

**Acceptance criteria**
- The account screen (doc 17) shows the user's friend code (`GET /api/sharing/me/profile`) with copy-to-clipboard.
- A "Friends" view lists accepted friends and pending incoming requests (`GET /api/sharing/friendships?status=`).
- "Add friend" takes a code (accepts with or without the `turbo-` prefix), looks it up (`GET /api/sharing/users/lookup?code=`), then sends a request (`POST /api/sharing/friendships/request`); a 404 on lookup shows "No user with that code".
- Accept (`POST /api/sharing/friendships/accept`) / decline or remove (`DELETE /api/sharing/friendships/{otherUserId}`) update the list optimistically and reconcile on refetch.
- A 409 on request ("already exists") is surfaced as "Request already sent / already friends" rather than a raw error.

**Web-specific notes**
- Friend code display + copy reuse the same clipboard helper as story 2.

### 5. Manage who a resource is shared with (and groups)
*As an owner, I want to see and manage all the people, groups, and links a resource is shared with, so that I control access.*

**Acceptance criteria**
- A "Manage sharing" view for an owned resource lists current grants (`GET /api/sharing/grants/resources/{resourceId}`): direct user grants, group grants, and link grants, each with role.
- The owner can revoke any grant (`DELETE .../users/{userId}`, `.../groups/{groupId}`, `.../links/{linkSubjectId}`); the list updates on success.
- The owner can grant directly to a friend (user grant) or a group (`POST /api/sharing/grants/users|groups`).
- Groups: create (`POST /api/sharing/groups`), rename (`PUT .../{id}/name`), delete (`DELETE .../{id}`), add member by friend code (lookup → `POST .../{id}/members`), remove member (`DELETE .../{id}/members/{userId}`); owner-only operations show a 403 gracefully if attempted on a non-owned group.
- Revoking a link grant invalidates the corresponding `/s/{token}` URL (story 3's "no longer available" path).

**Web-specific notes**
- Group/grant management is desktop-friendly (a side panel/dialog) and works on touch as a bottom sheet; no Android-specific affordance is required.

## Primary flows (web)

**Happy path — share a track**
1. On a synced track's detail panel, user clicks **Share**.
2. App confirms signed-in + synced; calls `POST /api/sharing/grants/links { resourceId, role:'viewer' }`.
3. Response `linkToken` → URL `https://<origin>/s/{token}` shown with **Copy** + native-share.
4. User copies and sends it.

**Happy path — open a shared link**
1. Recipient opens `/s/{token}`.
2. App calls `POST /api/sharing/grants/links/{token}/redeem` → `{ resourceId, resourceType, role }`.
3. App fetches the resource, builds a read-only Scene, fits camera to bounds, shows a minimal detail panel.
4. If signed in, the resource now appears under the recipient's "Shared" tab.

**Edge — unauthenticated sharer**
- "Share" → sign-in prompt (doc 17); after sign-in the user returns to the resource and can re-tap Share.

**Edge — resource not synced**
- "Share" → "Turn on cloud sync to share" prompt (doc 18); once synced (resource gets a server id) Share proceeds.

**Edge — link not found / revoked / expired**
- Redeem returns 404/410/403 → friendly "This shared link is no longer available" page with a button to the app home. No resource details are leaked.

**Edge — offline / network error on redeem**
- Show a retry state ("Couldn't load this shared link — Retry"), not a dead page.

**Edge — empty social graph**
- Friends/Groups/Shared tabs show empty states with the user's own friend code prominent and an "Add friend" CTA.

## UI / UX on web
- **Share dialog:** a modal/popover from the resource detail (desktop) or bottom sheet (touch) with the URL, Copy, and native-share.
- **Account → Friends/Groups/Shared:** a section reachable from the account screen (doc 17). On desktop a tabbed panel; on mobile a full-screen route with a segmented control (Friends / Groups / Shared) mirroring Android's tabs.
- **Manage sharing:** opened from an owned resource's overflow menu.
- **Public shared view (`/s/{token}`):** full-bleed `<TurboMapCanvas>` with minimal top bar (resource name + "Open Turbo" link) and a read-only detail panel; bottom sheet on mobile uses `set_viewport_inset` like other detail views.
- **Composition with canvas:** the public view fits camera to the resource via `ease_to` after computing bounds; no edit gestures bound beyond pan/zoom/orbit.

## Data & APIs
- **Create/redeem link:** `POST /api/sharing/grants/links` (body `{ resourceId, role, expiresAt? }`) → `{ resourceId, subjectId, linkToken, role, grantedAt, expiresAt }`; `POST /api/sharing/grants/links/{token}/redeem` → `{ resourceId, resourceType, role }`. *(create=auth; redeem auth TBD — see Open questions)*
- **Friend code / lookup:** `GET /api/sharing/me/profile` → `{ userId, friendCode, createdAt }`; `GET /api/sharing/users/lookup?code=` → `{ userId }` (404 if none). *(auth)*
- **Friendships:** `GET /api/sharing/friendships?status=`, `POST /friendships/request|accept`, `DELETE /friendships/{otherUserId}`. *(auth)*
- **Groups:** `GET|POST /api/sharing/groups`, `GET|DELETE /api/sharing/groups/{id}`, `PUT /api/sharing/groups/{id}/name`, `POST /api/sharing/groups/{id}/members`, `DELETE /api/sharing/groups/{id}/members/{userId}`. *(auth)*
- **Grants:** `GET /api/sharing/grants/resources/{resourceId}`, `POST /api/sharing/grants/users|groups`, `DELETE /api/sharing/grants/resources/{resourceId}/{users|groups|links}/{id}`. *(auth)*
- **Shared resources (Shared tab):** `GET /api/sharing/resources/sync?since=&types=` → `{ items: ResourceEnvelope[], serverTime }` (envelope: `id,type,ownerId,visibility,myRole,version,updatedAt,deleted`). *(auth)* — see doc 18.
- **State:** TanStack Query keys: `['sharing','profile']`, `['sharing','friendships',status]`, `['sharing','groups']`, `['sharing','grants',resourceId]`, `['shared','resources']`. Mutations (request/accept/remove/create-link/revoke) invalidate the relevant key. The public-view route keys redeem by `['share',token]`.
- **External:** none.

## Renderer integration
- **Public shared view:** build a `geo-json` source for the resource geometry + a `line`/`symbol`/`circle` layer (track line, marker symbol); for a raised track tube use the `set_route_tube` passthrough — **requires exposing `set_route_tube` in `turbomap-web`** (engine already implements it; same prerequisite as docs 08/09).
- **turbomap-web methods:** `apply_scene`, `ease_to`/`set_camera` (fit to bounds), `set_viewport_inset` (mobile detail sheet). No new passthrough beyond `set_route_tube`.

## Out of scope (this phase)
- Offline access to shared resources (the public view requires network).
- Email-based invites (`/api/sharing/invites/*`) and pending-invite redemption on sign-up — backend supports it, but the web phase scopes link + friend-code only.
- Push/notification when someone shares with you or sends a friend request.
- Editor-role collaborative editing of a shared resource (web phase is viewer-only sharing).

## Open questions
- **Anonymous redeem:** can a signed-out recipient redeem `/api/sharing/grants/links/{token}/redeem` and view, or does redeem require auth? This decides whether `/s/{token}` shows content directly or gates behind sign-in. (Android redeem runs authenticated.)
- **Share URL origin:** confirm the web public route path (`/s/{token}` vs `/share/{token}`) and that links generated by Android/iOS resolve to the same web origin.
- **Link expiry default:** do link grants expire by default (`expiresAt`)? Surface expiry in the share dialog if so.
- **Role on link:** always `viewer` for web, or allow `editor` links later?
- Should the "Shared" tab resources be openable on the map directly (read-only), reusing the public view, or only listed? (Assume openable.)
