# 04 — Vector overlays (trails, wave, wind, avalanche)

> Independent, toggleable overlay layers — hiking trails, wave-height, wind-flow,
> and avalanche-danger zones — composited over the basemap (and 3D terrain) by
> adding sources + layers to the Scene and re-applying it, without rebuilding the
> map.

## Status
- **Android (gold standard):** A **Layers** bottom sheet (`MapLayersSheet`) offers
  four independent overlay toggles, modeled by `OverlayId` in
  `core/model/.../domain/Models.kt`:
  - **Hiking trails** — "Marked routes · Waymarked Trails". Backed by the
    Waymarked Trails hiking raster `https://tile.waymarkedtrails.org/hiking/{z}/{x}/{y}.png`
    (PNG, maxZoom 18, "© Waymarked Trails, © OpenStreetMap contributors").
  - **Avalanche terrain** — "Slope steepness ≥27° · NVE". Backed by NVE
    *Bratthetskart* WMTS/ArcGIS raster
    `https://gis3.nve.no/arcgis/rest/services/wmts/Bratthet_2024/MapServer/tile/{z}/{y}/{x}`
    (PNG, maxZoom 17, "© NVE — Bratthetskart"). **Note the `{z}/{y}/{x}` order.**
  - **Wave height** — "Marine swell heatmap". **Declared but unwired** on Android
    (`overlayTiles()` returns `null`) — pending a keyed/commercial raster source.
  - **Wind** — "Animated flow field". **Declared but unwired** on Android, same
    reason.

  Each active overlay becomes a `raster-xyz` source + `raster` layer composited on
  top of the base layer. On the turbomap (wgpu) path this happens via
  `MapStyles.turbomapRasterSpecs(base, activeOverlays)` →
  `TurbomapScene` Scene-IR JSON (`"type": "raster-xyz"` + `"type": "raster"`),
  exactly the mechanism the web reuses. Toggle state (`activeOverlays:
  Set<OverlayId>`) is held **in memory only** on Android (resets each launch).
- **Web today:** Not implemented. `apps/web` builds a single base Scene
  (`buildBaseScene`) in `TurboMapCanvas.tsx` and never adds overlay sources/layers.
  There is no Layers panel and no overlay state slice.
- **Renderer/back-end prerequisites:**
  - Scene IR `raster` layers already render in the web wrapper, so **trails and
    avalanche need no renderer change** — they are extra `raster-xyz` sources +
    `raster` layers in the Scene.
  - Endpoints: Waymarked Trails (external, public, no auth), NVE Bratthet WMTS
    (external, public). Optionally the Turbo backend's generic resource MVT
    `/api/tiles/{resource}/tiles/{z}/{x}/{y}.mvt` for *named/marked* trails sourced
    from Nasjonalturbase — **confirm which resource id** (see open questions). MVT
    trails would render as **line** (+ optional **symbol**) layers, which require
    widening the web Scene IR `Layer` union (today only `raster`/`hillshade`; see
    §Renderer integration).
  - Wave + wind have **no wired data source** even on Android. Web ships them as
    disabled/"coming soon" toggles unless a keyed raster source (e.g. MET THREDDS
    raster, or a proxied tile endpoint) is provided.

## User stories

### 1. Open the layers panel
*As a hiker, I want to open a Layers panel, so that I can see which overlays are
available and which are currently on.*

**Acceptance criteria**
- A "Layers" affordance (map-overlay button) opens a panel listing each overlay
  with title + subtitle and a switch reflecting current on/off state.
- The panel shows the four overlays (Trails, Avalanche, Wave height, Wind);
  unwired overlays (Wave, Wind, until a source exists) render disabled with a
  "coming soon" hint rather than a dead toggle.
- Opening/closing the panel does not change the map camera or rebuild the Scene.

**Web-specific notes**
- On desktop the panel is a right-side or anchored popover; on touch it is a
  bottom sheet. When shown as a bottom sheet, call `set_viewport_inset(bottomPx)`
  so the user-centered content isn't hidden behind the sheet (same pattern as
  other docs).

### 2. Toggle each overlay independently
*As a user, I want to switch each overlay on or off independently, so that I see
exactly the information I want.*

**Acceptance criteria**
- Toggling an overlay **on** adds that overlay's `raster-xyz` source + `raster`
  layer to the current Scene and calls `apply_scene` — **without** tearing down
  the base layer, DEM/terrain, or other active overlays.
- Toggling an overlay **off** removes only that overlay's source + layer and
  re-applies the Scene; the map keeps rendering the same frame otherwise.
- Toggling does not reset the camera, re-fetch base tiles already in cache, or
  drop in-flight tile requests for other layers.
- Each overlay is fetched host-side through the existing `TileLoader` (the engine
  emits the overlay's tiles in `pending_tiles()` once its source is in the Scene).

**Web-specific notes**
- Overlay raster layers must be ordered **above** the base raster (and above
  hillshade/terrain shading) but the Scene `layers` array is drawn in order, so
  the compositor appends overlay layers after the base layer entry. Avalanche +
  trails together stack avalanche under trails (or by a defined precedence — see
  open questions).
- The NVE Bratthet source uses `{z}/{y}/{x}`; the `raster-xyz` `tiles` template
  must encode that axis order exactly (the engine substitutes `{x}/{y}/{z}`
  tokens literally).

### 3. Overlays compose with base layer and 3D
*As a user, I want overlays to keep working when I switch basemaps and when I tilt
into 3D, so that the experience is consistent.*

**Acceptance criteria**
- Switching the base layer (doc 01) rebuilds the base source but preserves active
  overlay sources/layers (the compositor reassembles the full Scene from current
  base + active overlays).
- In 3D (doc 02), raster overlays drape over DEM terrain like the basemap — no
  extra work beyond having the `raster` layers in the Scene.
- Turning on realistic water / hillshade (doc 03) does not disable overlays;
  overlay layers sit above shading layers.

**Web-specific notes**
- Overlays are raster, so they drape via the same terrain sampling as the base
  raster. If MVT trail lines are added later, line layers also follow the DEM in
  the engine.

### 4. Layer state persists for the session
*As a returning user, I want my overlay choices remembered while I use the app, so
that I don't re-toggle them on every navigation.*

**Acceptance criteria**
- Active overlay set survives in-app navigation (panel close/open, route to other
  screens and back) within the same session.
- Active overlay set is restored on reload within the session window
  (`localStorage`), matching/ exceeding Android (Android only holds it in memory).
- Restoring overlays on load adds their layers to the initial Scene before the
  first `apply_scene`, so the user doesn't see a flash of base-only map.

**Web-specific notes**
- Android keeps overlay state in-memory only; the web persists to `localStorage`
  as a deliberate (cheap) improvement. Server-side persistence is **out of scope**
  here (could later live under settings, doc 19).

## Primary flows (web)

**Happy path — enable trails + avalanche**
1. User taps the Layers button → panel opens (bottom sheet on touch; `set_viewport_inset` applied).
2. User flips **Hiking trails** on. The overlay store adds `trails` to the active
   set; the compositor rebuilds the Scene (base + DEM + `ov_Trails` source/layer)
   and calls `apply_scene`.
3. Engine reports new pending raster tiles for `ov_Trails`; `TileLoader` fetches
   `https://tile.waymarkedtrails.org/hiking/{z}/{x}/{y}.png` and ingests them.
4. User flips **Avalanche terrain** on → `ov_Avalanche` source/layer appended above
   `ov_Trails`; re-apply; NVE tiles fetch and ingest.
5. User closes the panel; both overlays remain drawn over the base + terrain.

**Switch basemap with overlays on**
1. With trails on, user changes base layer.
2. Compositor reassembles Scene = new base source/layer + existing `ov_Trails`;
   `apply_scene`. Trails persist; only base tiles change.

**Unwired overlay (Wave / Wind)**
1. User opens panel; Wave/Wind toggles are disabled with "coming soon".
2. If a keyed raster source is later configured, the same toggle path applies
   (add `raster-xyz` source + `raster` layer); no other code path changes.

**Network error on an overlay source**
1. Overlay tiles 404/timeout (e.g., Waymarked rate-limit). `TileLoader` treats
   404/204 as "tile absent" (no retry storm); transient errors retry on the next
   frame's `pending_tiles()` while the source stays in the Scene.
2. The base map keeps rendering; the overlay simply shows gaps until tiles arrive.
   The toggle stays "on"; no error modal blocks the map.

**Unauthenticated**
- Trails/avalanche overlays are public (external services) and do not require the
  Turbo session. If MVT trails via `/api/tiles/{resource}/...` are used and that
  resource is public, no auth is required either (tiles are listed public).

## UI / UX on web
- **Entry point:** a "Layers" button in the map-overlay control cluster (alongside
  base-layer + 3D toggles).
- **Panel:** desktop → anchored popover/side panel; touch → bottom sheet. Rows:
  title, subtitle, trailing switch. Disabled rows show a muted "coming soon" tag.
- **Map composition:** when shown as a bottom sheet, `set_viewport_inset(sheetPx)`
  keeps the focal point above the sheet. Closing resets the inset to 0.
- **Responsive:** pointer hover states on desktop switches; full-width tappable
  rows on touch.

## Data & APIs
| Overlay | Source | URL template | Type | Auth | Notes |
|---|---|---|---|---|---|
| Trails | Waymarked Trails (external) | `https://tile.waymarkedtrails.org/hiking/{z}/{x}/{y}.png` | raster PNG, maxZoom 18 | none | attribution required |
| Trails (alt/marked) | Turbo backend, Nasjonalturbase | `/api/tiles/{resource}/tiles/{z}/{x}/{y}.mvt` | MVT (line/symbol) | confirm | needs Scene IR line layer + resource id |
| Avalanche | NVE Bratthet WMTS (external) | `https://gis3.nve.no/arcgis/rest/services/wmts/Bratthet_2024/MapServer/tile/{z}/{y}/{x}` | raster PNG, maxZoom 17, **{z}/{y}/{x}** | none | "© NVE — Bratthetskart" |
| Wave height | none wired | — | raster (intended) | — | disabled until keyed/commercial source (e.g. MET) |
| Wind | none wired | — | raster (intended) | — | disabled; animated field is Android-deferred too |

- **Tile fetch:** all overlays flow through the existing host-driven `TileLoader`
  (`pending_tiles()` → `fetch` → `ingest_raster_tile`). No per-overlay fetch code.
- **State:** a Zustand slice `overlays: Set<OverlayId>` (mirrors Android's
  `OverlayId` enum) + persistence to `localStorage`. No TanStack Query needed for
  raster overlays (tiles are not JSON resources). If MVT trails are added,
  attribution/metadata could be a small query.
- **External sources:** Waymarked Trails, NVE. Both public; honor attribution in
  the map's attribution control.

## Renderer integration
- **Sources added per active overlay:** one `raster-xyz` source
  (`{ type: 'raster-xyz', tiles: [template], max_zoom }`), id `ov_<Name>`.
- **Layers added:** one `{ type: 'raster', id: 'ov_<Name>', source: 'ov_<Name>' }`,
  appended **after** the base raster layer in `Scene.layers` (draw order = array
  order).
- **Compositor:** a single React module builds the full Scene from
  `{ base, terrainOn, water/hillshade flags, activeOverlays }` and calls
  `apply_scene` once per change. (This is the same compositor introduced in doc
  05; overlays are just additional source/layer contributions.)
- **`turbomap-web` methods called:** `apply_scene` only. No new passthrough is
  required for raster overlays.
- **Scene IR widening (only if MVT trails are pursued):** the web `Layer` union in
  `apps/web/src/map/scene.ts` is currently `raster | hillshade`; add `line`
  (+ optionally `symbol`) variants to mirror `turbomap-scene` so MVT trail
  geometry can render. The engine already supports these layer types; this is a
  TS-type + Scene-builder change, not an engine change.

## Out of scope (this phase)
- Offline caching of overlay tiles / "download overlay for area".
- Server-side persistence of overlay preferences (session `localStorage` only).
- The animated **wind flow field** as a true particle animation — Android itself
  ships it unwired; web parity is a static raster at most, deferred until a data
  source exists.
- Wave-height + wind data sourcing/keys/proxy.

## Open questions
- **Marked trails source:** use external Waymarked Trails raster only, or also the
  Turbo backend MVT (`/api/tiles/{resource}/...`) for Nasjonalturbase marked
  routes? If MVT, **which `resource` id**, and is it public? (Drives the Scene IR
  line-layer work.)
- **Overlay stacking precedence** when multiple are on (trails over avalanche, or
  user-orderable?). Android appends in toggle order — confirm desired order.
- **Wave/wind data:** is there a sanctioned keyed source (MET THREDDS raster,
  proxied tiles) we can wire for web, or do these stay disabled like Android?
- **Waymarked Trails usage policy:** acceptable for production traffic, or proxy
  through the Turbo backend to add caching + respect rate limits?
