# 07 — Photos attached to markers

> Let a signed-in user attach photos to a marker, view them in the marker detail grid, and delete them — within the web's photo-capture constraints.

## Status
- **Android (gold standard):** From a marker's detail sheet → **Add photo** → choose **Camera** (`TakePicture()` → FileProvider temp file) or **Gallery** (`PickVisualMedia`, images only). Either way the photo is copied to app-private storage (`files/photos/{uuid}.jpg`) and **geotagged at the marker's coordinate**. The marker detail shows a horizontal grid of 72 dp thumbnails; tapping opens a full-screen swipeable viewer with a filmstrip and a delete button. Standalone geotagged photos (`markerId == null`) are **clustered on a "photo_map" layer** (`clusterPhotos`, ~90 m grid cells); tapping a cluster opens a grid sheet. Source: `feature/photos/.../AddMarkerPhotoSheet.kt`, `MarkerPhotos.kt`, `PhotoMapScreens.kt`, `PhotoClustering.kt`, `core/model/.../domain/Photo.kt`. **Photos are stored locally on Android today — there is no backend photo sync endpoint in the app.**
- **Web today:** Not implemented. No photo upload, storage, or display.
- **Renderer/back-end prerequisites:**
  - Marker detail sheet from `06-markers.md` (the grid lives in its body slot).
  - **Photo storage/upload endpoint is TBD** (see Open Questions) — Android stores photos locally and the sync wire does not carry them, so there is no existing server contract to mirror. Likely a per-marker photos endpoint or an object store with presigned upload.
  - For an optional manual photo pin: a `geo-json` + `symbol` layer (same overlay-compositor pattern as markers, `05`). No new renderer feature.

## Web capture constraint (state plainly)
The web **cannot** scan the device photo library or auto-geotag-and-cluster a user's camera roll. There is no browser API for background access to the OS photo library, and no reliable EXIF-GPS pipeline equivalent to Android's. Therefore on web:
- Photos are added **only by explicit user action**: `<input type="file" accept="image/*" capture="environment">` (mobile camera/picker), **drag-and-drop** of an image file, or **`getUserMedia` camera capture** (live capture in-page).
- Photos are **geotagged at the marker's coordinate** (the point they're attached to), **not** from image EXIF GPS.
- The Android **library-scan "photo_map"** (auto-clustering every geotagged photo on the device) is a **degraded / deferred capability on web** — it is explicitly **not** ported in this phase. Web users get per-marker photos and, optionally, manually-placed photo pins; they do **not** get an automatic photo map of their camera roll.

## User stories

### 1. Attach a photo to a marker (upload)
*As a signed-in user, I want to attach a photo to one of my markers, so that I can remember what the place looked like.*

**Acceptance criteria**
- From the marker detail sheet (`06`), **Add photo** offers: take a photo (mobile capture / `getUserMedia`), pick a file, or drag-drop an image onto the sheet.
- Selected image(s) upload to the photo storage endpoint (TBD) associated with the marker id; on success they appear in the marker's grid.
- The photo's location is **the marker's coordinate** (no EXIF read).
- Upload shows progress and is cancellable; a failed upload shows an inline retry and does not corrupt the existing grid.
- Reasonable client-side limits: image MIME types only; large images are downscaled/compressed client-side before upload (target a sane max dimension/size).

**Web-specific notes**
- `capture="environment"` opens the rear camera on mobile; on desktop the same `<input>` is a normal file picker. `getUserMedia` is the in-page live-capture path where supported (camera permission is a per-API browser prompt — no upfront gate).
- Multiple selection is allowed; uploads run as independent mutations so one failure doesn't block the rest.

### 2. View and delete photos in marker detail
*As a signed-in user, I want to see a marker's photos and remove ones I don't want, so that the marker stays tidy.*

**Acceptance criteria**
- The marker detail grid shows thumbnails (mirrors Android's 72 dp row); tapping a thumbnail opens a full-screen / lightbox viewer with swipe/next-prev navigation.
- The viewer has a **Delete** action (with confirm). Delete removes the photo from storage (TBD endpoint) and from the grid.
- **Empty state:** a marker with no photos shows an "Add photo" affordance and no empty grid chrome.
- Deletion failure leaves the photo in the grid and surfaces a retry.

**Web-specific notes**
- Thumbnails should use the storage service's thumbnail/resized variant if the endpoint provides one; otherwise the web downscales for the grid and loads full-res only in the viewer.

### 3. (Optional) place a manual photo pin on the map
*As a signed-in user, I want to drop a photo at a specific spot on the map (not tied to an existing marker), so that I can pin a memory to a location.*

**Acceptance criteria**
- Optional / lower priority. If shipped: long-press/right-click → "Add photo here" places a standalone photo at the chosen lat/lng (Android's `markerId == null` case), rendered as a photo pin on a dedicated `geo-json`/`symbol` layer.
- These standalone photo pins are **manually placed only** — there is no automatic camera-roll clustering (see constraint above). If several are placed near each other, a simple client-side cluster (mirroring `clusterPhotos`' grid approach) may group their pins, but the source set is only the user's explicitly-placed photos, never a library scan.

**Web-specific notes**
- Treat as a clearly-scoped extra; the core deliverable is per-marker photos. If storage for standalone (markerless) photos is unsettled, defer story 3 and ship stories 1–2.

## Primary flows (web)

**Attach (happy path):** marker detail → Add photo → capture/pick/drop image → client downscale → `POST` to photo endpoint (marker id) → on success append to grid + invalidate `['marker-photos', markerId]`.

**View/delete:** tap thumbnail → lightbox → Delete → confirm → `DELETE` photo → remove from grid.

**Unauthenticated:** photos hang off markers, which require auth; a signed-out user can't reach a marker detail to add a photo. No anonymous photo store this phase.

**Permission denied (camera):** if `getUserMedia`/`capture` permission is denied, fall back to the file-picker path and show a hint; never hard-block — drag-drop and file pick remain available.

**Network error:** upload failure → inline retry, original grid intact. Delete failure → photo stays, retry shown.

**Empty state:** marker with no photos → "Add photo" prompt only.

## UI / UX on web
- **Grid:** lives in the marker detail sheet body slot (`06`). Horizontal scroll of square thumbnails on mobile; a small wrapped grid in the desktop side panel.
- **Add photo:** an action-bar button / overflow item in the detail sheet, opening a small menu (Camera / Choose file). The sheet itself is a drag-drop target.
- **Viewer:** full-screen modal on mobile, centered lightbox on desktop, with swipe / arrow-key navigation and a Delete action.
- Camera permission is requested only when the user invokes capture (per-API prompt), matching the web platform model.

## Data & APIs
- **Marker association:** photos are keyed by marker id (from `06`, `/api/geo/locations/{id}`).
- **Photo storage/upload endpoint: TBD (open question).** Candidates: a per-marker photos sub-resource (e.g. `…/locations/{id}/photos`) or an object store with presigned `PUT` + a metadata record. Must support upload, list-by-marker, fetch (and ideally a thumbnail variant), and delete. **Confirm the contract before building.** Since Android stores photos locally only, there is no existing wire format to copy.
- **Auth:** required (cookie-based), inherited from the marker it attaches to.
- **TanStack Query keys:** `['marker-photos', markerId]` (list); upload/delete mutations invalidate it.
- **Zustand:** lightbox open/index, upload-progress state.

## Renderer integration
- **Per-marker photos** are pure UI (HTML `<img>` in the sheet/lightbox) — no renderer involvement.
- **Optional manual photo pins (story 3):** a `geo-json` source + `symbol` layer composed into the Scene via `apply_scene`, same pattern as the marker layer in `06`/`05`. Hit-test via `unproject` (or `hit_test` once exposed). No new engine feature.

## Out of scope (this phase)
- Device photo-library scanning and the **auto-geotag "photo_map" clustering** of the camera roll — degraded/deferred on web by platform constraint (stated above).
- EXIF-GPS-based geotagging (web geotags at the marker/tap point only).
- Offline photo capture/queue and crash-recovery of in-progress uploads (offline/PWA phase).
- Photo sharing / albums / collections of photos (see `15`/`16` if/when defined).

## Open questions
1. **Storage/upload endpoint — primary blocker.** Confirm the real contract: per-marker sub-resource vs object store + presigned URL; max size; thumbnail generation; whether photos sync across devices (Android keeps them local). This must be settled before implementation.
2. **Standalone (markerless) photo pins.** Ship story 3 this phase, or defer until the storage model is confirmed?
3. **Retention / quota.** Per-user storage limits and lifecycle (deleted with the marker?) for server-stored photos.
4. **Thumbnail strategy.** Server-generated thumbnails vs client downscale for the grid.
