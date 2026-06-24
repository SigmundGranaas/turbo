# Design brief — Turbo web app

> Hand-off instruction for the design pass (do this **before** implementation).
> Goal: design the UI/UX for the new **Turbo web app** — a React app whose map is
> a full-screen 3D **wgpu renderer** (the same engine as the native apps). The
> native **Android "Expressive" app is the gold standard**; the web should feel
> like a first-class member of that family, adapted to web-native interaction.

## What to design

The complete visual + interaction design for the web app: a **design-token set**,
a **component library**, and **key screen/flow frames** for both **desktop
(pointer)** and **mobile-web (touch)**. Dark-first (the map is the hero and the
shell is dark), with a light theme as a parallel set.

## Source of truth (read these first)

- **Feature scope + user stories:** `docs/architecture/web-port/` — one doc per
  feature area (`01`…`19`, README is the index). Each has detailed user stories,
  flows, and web constraints. **Design every surface those stories imply.**
- **Visual gold standard:** the native app at `apps/android` (Jetpack Compose,
  **Material 3 Expressive**). Match its identity — terracotta primary palette,
  large rounded "squircle" shapes, the rounded **"Cookie"** avatar/badge motif,
  expressive type scale, `TurboCard`/`StatTile` patterns, Roboto Flex. Pull tokens
  from its theme (`:ui:theme`) rather than inventing a new language.
- **Current web shell (baseline to evolve):** `apps/web/src` — `MapScreen.tsx`
  (top bar + full-bleed map), `styles.css` (dark tokens already stubbed).

## The defining constraint: the map is the canvas

The 3D map fills the viewport; **all UI floats over it**. Design accordingly:
- **Map-overlay controls** (top): a search field, layers control, 2D⇄3D toggle,
  sun/water toggles, recenter-on-me, zoom. Glassy/translucent over terrain.
- **A bottom-sheet (mobile) / side-panel (desktop) system** is the primary
  surface for detail + lists. Sheets are **multi-detent** (peek / half / full)
  like the Android live sheet. Specify how a sheet insets the map (the renderer
  shifts the camera up via a viewport inset, so the focused point stays visible).
- Keep the map legible: minimise chrome, prefer floating cards/sheets over
  full-screen takeovers except for auth/settings.

## Surfaces & flows to cover (from the port docs)

1. **App shell** — responsive frame, top bar (brand, search, account), the
   overlay control cluster, and the sheet/panel container. Desktop = persistent
   left/right panel; mobile = bottom sheet + FAB-ish controls.
2. **Map controls** — base-layer picker (Norgeskart/OSM/Satellite), overlay
   toggles (trails, wave, wind, avalanche), 2D/3D, sun time-of-day slider, water.
3. **Search** — field + fused results list (coordinates, markers, places, trails),
   filter chips, recents, empty/no-result/error states.
4. **Markers** — create flow (long-press/right-click → name sheet), detail sheet
   (name, kind, coords, notes, live weather, photo grid), edit, delete-confirm.
5. **Photos** — upload control + photo grid + lightbox (explicit upload only).
6. **Saved paths** — list (filter by source), detail sheet with **elevation
   profile chart**, import/export, rename, show-on-map, follow.
7. **Routing** — the unified Create-Route tool: mode tabs (Route/Line/Draw),
   stops list (reorder/remove), preset picker, **live-preview** state, stats
   strip (distance/duration/ascent/on-trail%/surface), save dialog.
8. **Follow / navigation** — live panel: progress bar + arc cursor, ETA/next
   checkpoint, speed/elevation, off-route "rerouting" state, arrival.
9. **Conditions** — sheet with Weather / Ocean / Avalanche tabs; weather-now card,
   hourly+daily forecast, marine+tides, avalanche level card; a long-press
   weather popover; a "conditions along route" strip.
10. **Activities** — kind picker (18 kinds w/ icons), kind-colored map pins.
11. **Collections** — list (name + color + count), create/rename/recolor, detail.
12. **Sharing** — share dialog (link + copy), friend code, friends/groups view,
    and the **public read-only shared view** a recipient opens.
13. **Account / auth** — login + register (email/password) and Google button;
    account screen (email, friend code, sync status, sign out).
14. **Settings** — theme, units, compass, sync toggle, language, about.
15. **System states** — loading/skeletons, empty states, error toasts, the
    **WebGPU-unsupported** full-screen notice, signed-out (read-only) state.

## Design tokens to define

Color (dark + light; terracotta primary + activity swatches), typography scale,
corner radii (the expressive squircle family), spacing, elevation/blur for
floating glass surfaces, motion (sheet detents, fly-to, control transitions),
iconography set, and the map-overlay treatment (scrim/contrast so controls read
over both bright snow and dark terrain).

## Out of scope (do NOT design)

- **Recording** — removed from the web scope; no record button, live recording
  sheet, or lock-screen widgets.
- **Offline** — no offline-region download UI, "download along route", or
  wifi-only settings (deferred to a later phase).
- Background-tracking / notification / lock-screen surfaces (browser can't).

## Deliverables

1. **Token set** (Figma variables / styles) mapped from the Android theme.
2. **Component library** — buttons, chips, cards, sheets/detents, list rows,
   the search field, map control cluster, stat tiles, elevation chart, tabs,
   dialogs, toasts, the Cookie avatar.
3. **Key frames** for the surfaces above, in **desktop + mobile-web** widths,
   including dark + light and the important states (empty/loading/error).
4. A short rationale on how the web adapts Android's Expressive language to
   pointer + keyboard + responsive web.

## Hand-off back to engineering

Provide the frames + tokens + (ideally) Code Connect mappings so the React
components can be built against them. Engineering will implement feature-by-
feature against the port docs in this folder; the foundation feature is
`05-location-and-entities.md` (the shared overlay/scene compositor) — design the
map-overlay + sheet system with that in mind.
