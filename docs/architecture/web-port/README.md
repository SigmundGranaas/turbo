# Web port — feature port documents

This folder holds one **port document per feature area** for bringing the Turbo
web app (`apps/web`, React + the turbomap wgpu renderer compiled to WASM) up to
parity with the **native Android "Expressive" app** (`apps/android`), which is
the gold standard. Each doc describes the **user stories on the web** in detail:
what the user does, the flows, the web-specific UX, and how it maps onto the
existing backend + renderer.

**Scope: online features only.** Offline tile downloads, "download along route",
wifi-only limits, and crash-recovery draft persistence are explicitly out of
scope for these docs (a later offline/PWA phase covers them). Where a feature is
structurally weaker in a browser than on Android (background GPS, device photo
library), the doc says so plainly rather than pretending parity.

## Current web baseline (Phase 1)

Already built: Norgeskart raster basemap, 2D pan + wheel-zoom, host-driven tile
fetch, and auth plumbing (Google redirect, `/session/me`, logout). Everything
else below is a gap. See `TurboMapCanvas.tsx`, `api/`, `store/uiStore.ts`.

## The documents

| # | Doc | Area |
|---|-----|------|
| 01 | `01-base-layers.md` | Base layers + basemap switching |
| 02 | `02-3d-camera-terrain.md` | 3D orbit/tilt camera + DEM terrain |
| 03 | `03-map-realism.md` | Realistic water, sun/time-of-day, clouds, hillshade |
| 04 | `04-vector-overlays.md` | Trails, wave, wind, avalanche overlay layers |
| 05 | `05-location-and-entities.md` | My-location, heading, camera persistence, on-map entity rendering |
| 06 | `06-markers.md` | Markers/POIs CRUD + detail |
| 07 | `07-photos.md` | Photos attached to markers |
| 08 | `08-saved-paths.md` | Saved paths/tracks list, detail, elevation, import/export |
| 09 | `09-routing.md` | Route planning, presets, SSE live preview, stops, line/draw |
| 12 | `12-search.md` | Search: coords, markers, places, trails, recents |
| 13 | `13-conditions-weather.md` | Weather, avalanche, marine, tides, sun |
| 14 | `14-activities.md` | Activity kinds + assignment |
| 15 | `15-collections.md` | Collections of markers/tracks |
| 16 | `16-sharing-social.md` | Share links, friend code, social graph |
| 17 | `17-account-auth.md` | Login (email/password + Google), account screen |
| 18 | `18-sync.md` | Cloud delta-sync of markers/tracks/collections |
| 19 | `19-settings.md` | Theme, units, compass, sync toggle, about, i18n |

## Shared context (every doc assumes this)

### Stack
- **React 19 + TypeScript + Vite** (`apps/web`). Server state via **TanStack
  Query**; client/UI/session state via **Zustand** (`store/uiStore.ts`).
- **Renderer**: `turbomap-web` (WASM) — `<TurboMapCanvas>` owns the `TurboMap`
  instance, the rAF render loop, host-driven tile fetch (`TileLoader`), and
  pointer/wheel gestures. Map-visual state lives in the engine; React feeds it
  input, tiles, and a Scene.

### Backend (`https://kart-api.sandring.no`, base = `VITE_API_BASE`)
CORS allows the dev origin with credentials. Auth is **cookie-based** for the web
(HttpOnly `.sandring.no` cookies; `apiFetch` uses `credentials: 'include'`).

- **Auth**: `POST /api/auth/auth/login`, `/register`; `GET /api/auth/oauth/google/url`
  → callback sets cookies; `GET /api/auth/session/me`; `POST /api/auth/token/refresh`,
  `/revoke`.
- **Markers (locations)**: `/api/geo/locations` — CRUD, bbox query, delta-sync
  `?since=`, `If-Match` optimistic concurrency. *(auth required)*
- **Tracks**: `/api/tracks/tracks` — CRUD, delta-sync `?since=`. *(auth)*
- **Collections**: `/api/collections/*`. *(auth)*
- **Sharing**: `/api/sharing/resources/sync?since=&types=`, grant/link/friendship/
  group endpoints. *(auth)*
- **Activities**: `/api/activities/{kind}/conditions|observations|activities`.
- **Places (geocode, public)**: `GET /api/places/search?q=&lat=&lon=&limit=`,
  `GET /api/places/reverse?lat=&lon=`.
- **Routing (public)**: `POST /api/route/plan`, `POST /api/route/plan/stream` (SSE),
  `GET /api/route/presets`. Params: `profile` (foot/bicycle/ski), `preset`
  (balanced / avoid_roads / direct / easy_grade / trail_purist).
- **Tiles (public)**: raster `/api/tiles/raster/n50/{z}/{x}/{y}.png`; vector basemap
  `/api/tiles/basemap/{z}/{x}/{y}.mvt` (+ `style.json`); DEM `/api/tiles/dem/rgb/{z}/{x}/{y}.png`;
  slope `/api/tiles/slope/tiles/{z}/{x}/{y}.png`; generic resource MVT
  `/api/tiles/{resource}/tiles/{z}/{x}/{y}.mvt`; `elev` sample/profile POSTs;
  fonts + sprite.
- **External (via app or proxied)**: MET (weather / ocean / radar), Varsom/NVE
  (avalanche), Kartverket (stedsnavn place search, høydedata elevation, tides,
  reverse geocode), Nasjonalturbase (named trails).

### turbomap-web API (the WASM map handle; `pkg/turbomap_web.d.ts`)
`TurboMap.create(canvas,w,h,lat,lng,zoom)` (async) · `apply_scene(json)` ·
`pending_tiles()` → `[{kind,layer,z,x,y}]` · `ingest_raster_tile` /
`ingest_terrain_tile` / `ingest_vector_tile` · `render()` · `tick()` ·
`is_animating()` · `resize()` · `set_camera(lat,lng,zoom,pitch,bearing)` ·
`camera_json()` · `pan_by_pixels` · `zoom_around` · `orbit_around` · `ease_to` ·
`project(lat,lng)` / `unproject(x,y)` · `set_viewport_inset` ·
`set_terrain_shadows` · `set_realistic_water` · `set_sun_time`.

> **Renderer gap to call out where relevant:** the Android FFI exposes more than
> the current web wrapper — `set_route_tube`, `set_water_conditions`,
> `enable_clouds`/`ingest_radar_frame`/`set_cloud_time`, `hit_test`,
> `set_cloud_geo_bounds`. Docs that need these must note "**requires exposing X
> in `turbomap-web`**" (a thin wasm-bindgen passthrough over the existing engine
> method — the engine already implements it).

### Scene IR (`turbomap-scene`, mirrored in `apps/web/src/map/scene.ts`)
`Scene { sources: Record<id, SourceDef>, layers: Layer[] }`. Sources:
`raster-xyz`, `vector-xyz`, `geo-json` (inline; drains in-process), `dem-xyz`.
Layers: `raster`, `hillshade`, `fill`, `line`, `symbol`, `circle`, … Overlays
(routes/tracks/markers as data) are added by composing `geo-json` sources +
line/fill/symbol/circle layers and calling `apply_scene`, OR (for raised route
tubes) via the `set_route_tube` passthrough once exposed.

### Web platform constraints (recurring)
- **SSE** (route streaming): use `fetch` + `ReadableStream` (the endpoint is a
  POST, so `EventSource` won't work) — mirror Flutter's `fetch_client` approach.
- **Live location**: `navigator.geolocation.watchPosition`. **Background GPS is
  unreliable** when the tab is hidden → recording/follow run "while the app is
  open + screen on". No foreground service / lock-screen widget (Android-only).
- **Photos**: `<input type=file capture>` / drag-drop / `getUserMedia` only — no
  device photo-library scanning or auto-geotag pin clustering.
- **WebGPU required**; show a graceful unsupported-browser notice (Chrome/Edge,
  Safari 18+, Firefox w/ flag).
- **Permissions** are per-API browser prompts (geolocation, camera) — no upfront
  onboarding permission gate like Android.

## Per-document template (authors: follow this exactly)

```md
# <NN> — <Feature area>

> One-sentence purpose.

## Status
- **Android (gold standard):** <what the native app does today>
- **Web today:** <what apps/web has now — usually "not implemented">
- **Renderer/back-end prerequisites:** <e.g. "expose set_route_tube in turbomap-web", endpoints used>

## User stories
Numbered, detailed. Each: `As a <user>, I want <capability>, so that <value>.`
Follow each with **Acceptance criteria** (bullet list, testable) and any
**web-specific notes** (constraints/degradations vs Android).

## Primary flows (web)
Step-by-step walkthroughs of the main interactions (happy path + key edge cases:
unauthenticated, offline-network error, permission denied, empty state).

## UI / UX on web
Where it lives in the app shell (top bar, side panel, bottom sheet, map overlay),
responsive notes (desktop pointer vs touch), and how it composes with the map
canvas (e.g. viewport inset for a bottom sheet via `set_viewport_inset`).

## Data & APIs
Endpoints used (with method + key params), request/response shape if known,
auth requirement, TanStack Query keys / Zustand slices, and external sources.

## Renderer integration
Scene sources/layers added, `turbomap-web` methods called, any passthrough that
must be added to the WASM wrapper first.

## Out of scope (this phase)
Offline + any explicitly deferred sub-features.

## Open questions
Decisions to confirm with the product owner.
```

Keep each doc concrete and scannable. Depth over breadth: real flows and
acceptance criteria, not vague summaries.
