# 01 — Base layers + basemap switching

> Let a web user pick which basemap the turbomap renderer draws under everything else — topo, street, or aerial — and remember that choice across sessions.

## Status
- **Android (gold standard):** Three base layers selectable from a **Layers** bottom sheet:
  - **Norgeskart** — Kartverket topographic raster (the default).
  - **OpenStreetMap** — global street raster.
  - **Satellite** — Kartverket / aerial imagery raster.
  The active layer is the single source the renderer draws under all overlays; the choice is **persisted** (DataStore) and restored on next launch. Switching is instant — only the base source swaps, overlays/markers/camera stay put.
- **Web today:** Norgeskart raster only. `scene.ts` hard-codes `BaseLayerId = 'norgeskart'`; `buildBaseScene` has a single `case`. `uiStore` already holds `baseLayer` + `setBaseLayer`, but there is no picker UI, no other layers, and no persistence.
- **Renderer/back-end prerequisites:**
  - **No renderer change needed.** Each base layer is just a `raster-xyz` (or `vector-xyz`) source swapped via `apply_scene`. The engine already diffs scenes and does minimal GPU work on swap.
  - Add the new `BaseLayerId` values + `buildBaseScene` cases in `scene.ts`, and the matching tile templates in `templates.ts`.
  - **Tile endpoints to confirm** (see Open questions): OSM raster URL (must be a usage-policy-compliant / self-hosted tile source, not `tile.openstreetmap.org` directly) and the Kartverket satellite/aerial XYZ endpoint. Norgeskart already proxies through `${API_BASE}/api/tiles/raster/n50/{z}/{x}/{y}.png`.
  - Persistence: web has no DataStore. Use **server settings** (account-scoped) when authenticated, falling back to **`localStorage`** so it works logged-out and is instant on boot.

## User stories

### 1. Switch the base layer
*As a map user, I want to switch between Norgeskart, OpenStreetMap, and Satellite, so that I can read the terrain the way that suits my task (topo for hiking, street for towns, aerial for ground truth).*

**Acceptance criteria**
- A Layers control is reachable from the map shell; opening it lists the three base layers with a name, a thumbnail/preview, and a selected indicator on the current one.
- Selecting a layer swaps the rendered basemap within one render frame after its first tiles arrive; the previously-rendered tiles stay visible until the new ones load (no white flash).
- Camera (center/zoom/pitch/bearing), any overlays, and the my-location dot are unchanged across the swap — only the base source changes.
- The control reflects the active layer immediately on selection (optimistic), before tiles finish loading.

**Web-specific notes**
- Swap is implemented by rebuilding the `Scene` for the new `BaseLayerId` and calling `apply_scene`; the host then services the new source's `pending_tiles()`. The `TileLoader` keys by `kind/layer/z/x/y`, so two different raster basemaps with the same `layer` id (`basemap`) will reuse cache slots — give each base layer a stable source/layer id or version the cache key so stale tiles aren't served after a swap.

### 2. Persist the choice across launches
*As a returning user, I want the app to reopen with the basemap I last chose, so that I don't re-pick it every visit.*

**Acceptance criteria**
- Choosing a layer persists it; reloading the tab (or reopening later) restores that layer before the first frame, so the user never sees the default flash to their choice.
- Logged-in users get the choice synced to their account settings and see it on any browser; logged-out users get it from `localStorage` on the same browser.
- If a stored layer id is unknown (e.g. a removed layer), fall back to `norgeskart` without erroring.

**Web-specific notes**
- Read `localStorage` synchronously during `uiStore` init so the initial `buildBaseScene` already uses the right layer (avoids a Norgeskart→chosen reflow). The server-settings read (TanStack Query) reconciles afterward and corrects only if it differs.

### 3. Layer picker UI
*As a touch and desktop user, I want a clear, one-tap Layers picker, so that switching is obvious and fast on both phone and laptop.*

**Acceptance criteria**
- On narrow/touch viewports the picker is a bottom sheet; on wide/pointer viewports it is a popover/panel anchored to a map rail button.
- Each option shows a label and a small static preview tile so users recognize the look without switching.
- The sheet/popover is keyboard navigable and dismissible (Esc / scrim tap / outside click).
- Opening the sheet on mobile applies a `set_viewport_inset` equal to the sheet height so the map content (and my-location dot) shifts up out from under it.

**Web-specific notes**
- This is base-layers only. Overlay toggles (trails, wave, wind, avalanche, hillshade) live in doc 04 / 03 and may share the same Layers sheet later, but this doc scopes only the single-select basemap row.

## Primary flows (web)

**Happy path — switch + persist**
1. User taps the **Layers** rail button → sheet/popover opens (`set_viewport_inset` applied on mobile).
2. User selects **Satellite**. `uiStore.setBaseLayer('satellite')` fires; the row shows selected immediately.
3. `TurboMapCanvas` reacts to `baseLayer`, rebuilds the scene via `buildBaseScene('satellite', tileUrl)`, calls `apply_scene`.
4. The rAF loop drains `pending_tiles()`; `TileLoader` fetches the satellite XYZ tiles and `ingest_raster_tile`s them. Old tiles stay until the new ones fade in.
5. Persistence: write `satellite` to `localStorage`; if authenticated, PATCH the settings endpoint (see doc 19).

**Edge — unauthenticated**
- Picker works fully; persistence is `localStorage`-only. No login prompt is shown for changing basemap.

**Edge — tile fetch network error**
- A failed base tile leaves that tile blank/last-good; the picker still shows the new layer as selected. Retry/backoff is the `TileLoader`'s job. No modal error for a basemap that can't load — just degraded tiles, with a non-blocking toast if *all* base tiles for the viewport fail.

**Edge — empty/first load**
- On first ever load with no stored choice, `norgeskart` is used and the picker highlights it.

## UI / UX on web
- **Where:** a **Layers** button on the map's right-side rail (mirrors Android's rail). Opens a bottom sheet (touch) or anchored popover (pointer).
- **Composition with canvas:** mobile sheet uses `set_viewport_inset(sheetHeightPx)`; closing resets it to `0`.
- **Responsive:** desktop popover does not inset the viewport (it overlays a corner).
- Active-layer state is the single `uiStore.baseLayer` slice; the canvas and the picker both read it (one-state-one-widget).

## Data & APIs
- **Tiles (public):**
  - Norgeskart: `GET ${API_BASE}/api/tiles/raster/n50/{z}/{x}/{y}.png` (already wired).
  - OpenStreetMap: **TBD** raster XYZ URL (self-hosted/proxied; not the public OSM tiles per usage policy).
  - Satellite: **TBD** Kartverket aerial/satellite XYZ URL.
- **Persistence (auth):** account settings via the settings endpoint (doc 19) — store `baseLayer`. **Logged-out:** `localStorage` key (e.g. `turbo.baseLayer`).
- **State:** Zustand `uiStore.baseLayer` / `setBaseLayer` (exists). TanStack Query key for server settings owned by doc 19; this doc only reads/writes the `baseLayer` field.

## Renderer integration
- **Scene sources:** one `raster-xyz` (or `vector-xyz`) per base layer, set as the lowest layer. Extend `SourceDef`/`buildBaseScene` switch in `scene.ts`.
- **Templates:** add `raster.osm` and `raster.satellite` (or distinct layer ids) entries in `templates.ts`; map each `BaseLayerId` to its template + source id.
- **turbomap-web methods:** `apply_scene` (swap), `set_viewport_inset` (sheet). **No new wasm passthrough required.**

## Out of scope (this phase)
- Offline tile pre-download / pinning of any base layer.
- Custom user-added tile sources or third-party basemap accounts.
- Overlay layers (trails, wave/wind/avalanche, hillshade) — see docs 03 and 04.

## Open questions
- **OSM tiles:** which URL? A self-hosted/proxied raster endpoint is needed; the public `tile.openstreetmap.org` violates the usage policy for an app. Should we proxy through `${API_BASE}/api/tiles/...`?
- **Satellite tiles:** exact Kartverket aerial/satellite XYZ endpoint, projection (Web Mercator vs UTM), zoom range, and attribution string.
- Should the base-layer choice be **per-device** (localStorage only) or **synced** across the user's devices via account settings? (Android uses local DataStore = per-device.)
- Min/max zoom + `tile_size` for OSM and Satellite (Norgeskart uses 256px, z0–18).
