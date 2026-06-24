# 14 — Activity kinds + assignment

> A shared registry of activity kinds (Mountain, Hiking, Kayaking, …) that categorise markers and tracks, drive their icon/colour, and gate activity-specific conditions.

## Status
- **Android (gold standard):** An `ActivityKindId` enum of **18 kinds** (`core/model/.../domain/Models.kt`), each with a stable Norwegian DB key and a Material Symbols icon (`core/designsystem/.../ui/theme/ActivityKindVisuals.kt`). A kind is **assigned on marker creation** (icon grid in the editor, default `Cabin`) and **optionally on saved tracks** (`PathEntity.activityKind`, nullable). The kind is persisted on the sync wire as `display.icon`. **Marker colour is a separate, user-chosen swatch** (Auto/primary + 6 ARGB presets), **not** a per-kind tint — every marker defaults to the single terracotta primary (`#8F4C38` light / `#FFBAA0` dark) unless the user overrides it. Kind surfaces in **search results** (markers carry their kind for display/filtering); there is no dedicated marker-kind filter screen in the current code. Source: `Models.kt`, `ActivityKindVisuals.kt`, `feature/map/.../markers/MarkerSheets.kt`, `feature/search/.../SearchViewModel.kt`.
- **Web today:** Not implemented. No activity-kind registry, no kind picker, no kind-aware paint.
- **Renderer/back-end prerequisites:**
  - Marker layer paint (`06-markers.md`) consumes a per-feature colour resolved from kind/colour; no new renderer feature.
  - Track create/edit (`08-saved-paths.md`, `09-routing.md`) reuses the same picker.
  - Activity conditions endpoints — see Open Questions (not found in the Android client today).

## The 18 activity kinds (exact, in order)
The TS registry must mirror `ActivityKindId` exactly (id, DB key, icon, label):

| # | Id | DB key (NO) | Android icon (Material Symbols) |
|---|----|-------------|---------------------------------|
| 1 | `Mountain` | Fjell | Landscape |
| 2 | `Park` | Park | Park |
| 3 | `Beach` | Strand | BeachAccess |
| 4 | `Forest` | Skog | Forest |
| 5 | `Hiking` | Vandring | Hiking |
| 6 | `Kayaking` | Kajakk | Kayaking |
| 7 | `Biking` | Sykkel | DirectionsBike |
| 8 | `Cabin` | Hytte | Cabin |
| 9 | `Parking` | Parkering | LocalParking |
| 10 | `Camping` | Camping | AirportShuttle |
| 11 | `Swimming` | Badeplass | Pool |
| 12 | `Diving` | Dykking | ScubaDiving |
| 13 | `Viewpoint` | Utkikkspunkt | PhotoCamera |
| 14 | `Restaurant` | Restaurant | Restaurant |
| 15 | `Cafe` | Kafé | LocalCafe |
| 16 | `Accommodation` | Overnatting | Hotel |
| 17 | `Fishing` | Fiskeplass | Phishing |
| 18 | `Skiing` | Ski | DownhillSkiing |

> Note: the enum id is `Cafe` (no accent); the user-facing label can be "Café". The DB key is the load-bearing wire value (stored as `display.icon`) and must match Android byte-for-byte.

## Colour model — read before "color-coding"
The prompt's "each kind has a default terracotta tint" is **not** how Android works today, and the web should match Android:
- Every marker defaults to the **single app primary** terracotta (`#8F4C38` light / `#FFBAA0` dark), regardless of kind.
- The user may override with one of **7 swatches**: `Auto` (= primary, value `null`) plus `#E0432B` (red), `#EF6C00` (orange), `#2E7D32` (green), `#1A73E8` (blue), `#7B1FA2` (purple), `#00838F` (teal). Stored as `colorArgb` (ARGB int), **local-only** (see `06`).
- So "kind colour-codes the pin" means: **the kind selects the pin's icon glyph; the colour comes from the user's swatch (or the shared primary), not from the kind.** A per-kind palette is a possible web enhancement, but shipping it would diverge from Android — call it out, don't assume it.

## User stories

### 1. Pick a kind when creating/editing a marker
*As a signed-in user, I want to choose what kind of place a marker is, so that its icon reflects what's there and I can find it later.*

**Acceptance criteria**
- The marker editor (`06`) shows an icon grid of all 18 kinds; the selected kind is highlighted; default on create is `Cabin`.
- The chosen kind is saved as `display.icon` (the DB key) via `/api/geo/locations`.
- The marker layer renders that kind's glyph on the pin.

### 2. Pick a kind for a saved track (optional)
*As a user, I want to tag a track with an activity, so that my tracks are categorised like my markers.*

**Acceptance criteria**
- Track create/edit (`08`/`09`) shows the same picker, but kind is **optional** (no default; nullable to match `PathEntity.activityKind`).
- The chosen kind persists with the track and can drive its list-row icon.

### 3. See kind-coloured / kind-iconed pins
*As a user, I want my markers visually distinguished by what they are, so that I can read the map at a glance.*

**Acceptance criteria**
- Each marker pin shows its kind's icon glyph.
- Pin colour follows the user's swatch (or the shared primary), per the colour model above — **not** a per-kind tint, to match Android.
- The registry is the single source of truth for id → icon → label used by the marker layer paint, the picker, list rows, and search.

### 4. Filter by activity kind
*As a user, I want to narrow my markers (or search) to a kind, so that I can find all the fishing spots / cabins.*

**Acceptance criteria**
- Filtering is exposed where Android exposes it: in **search results** (markers carry their kind), the result list can filter/group by kind.
- Android has **no dedicated standalone marker-kind filter screen** today; the web matches that scope — kind filtering lives in search (`12-search.md`), not as a separate map filter UI. A map-level kind filter is a noted future enhancement, not parity.

**Web-specific notes**
- The kind picker is a reusable React component fed by the registry; reuse it in marker editor, track editor, and search filter.

## Primary flows (web)
**Assign on marker create:** open editor → tap a kind in the icon grid (default `Cabin`) → Save → `display.icon = <DB key>` → pin renders that glyph.

**Assign on track:** track editor → optional kind picker → persist with track.

**Filter:** search → results list → filter chips by kind (kinds drawn from the registry).

**Empty / no-kind:** a track with no kind renders a neutral default icon; a marker always has a kind (defaults to `Cabin`).

## UI / UX on web
- **Picker:** a wrapping icon grid (FlowRow equivalent) of the 18 kinds with labels on hover/long-press; selected state highlighted. Embedded in the marker editor and track editor; a compact chip variant for search filters.
- **Registry component** drives icon + label everywhere; on web the Material Symbols glyphs map to an equivalent web icon set (Material Symbols web font or an inline SVG set) — keep the glyph names aligned with `ActivityKindVisuals.kt`.
- Responsive: full grid in the editor sheet; horizontally scrollable chips on narrow widths.

## Data & APIs
- **Registry:** a TypeScript module (e.g. `apps/web/src/activities/kinds.ts`) exporting an ordered array of `{ id, key, label, icon }` for the 18 kinds, mirroring `ActivityKindId`. The `key` is the wire value written to `display.icon` and read back on sync.
- **Markers:** kind round-trips via `/api/geo/locations` `display.icon` (see `06`/`18`). Auth required.
- **Tracks:** optional kind on `/api/tracks/tracks` (see `08`). Auth required.
- **Activity conditions/observations:** the README references `/api/activities/{kind}/conditions|observations|activities`. **These were not found in the Android client** (Android fetches conditions by coordinate from MET/NVE, not by kind). Treat activity-specific observation/condition forms as a **noted future sub-feature** — confirm the endpoints exist before building (Open Questions). If they do, an "activity conditions" view keyed by the selected kind can be layered on later.
- **TanStack Query / Zustand:** the registry is static (no query). Selected-kind lives in the editor's local/Zustand state.

## Renderer integration
- No new renderer feature. The marker `geo-json`/`symbol` layer (`06`) reads kind → glyph from the registry and colour → swatch/primary, composed via `apply_scene`. Tracks reuse their existing line layers; kind only affects list/detail iconography, not track geometry.

## Out of scope (this phase)
- Per-kind colour palettes (would diverge from Android; noted as a possible enhancement).
- A standalone map-level kind filter (Android has none today; kind filtering stays in search).
- Activity-specific observation forms and per-kind conditions views — deferred future sub-feature pending endpoint confirmation.
- Offline behaviour.

## Open questions
1. **Activity conditions endpoints.** Do `/api/activities/{kind}/conditions|observations|activities` actually exist server-side? They are not used by the Android client. If yes, define the contract for a future per-kind conditions view; if no, drop the reference.
2. **Per-kind colour.** Match Android (single primary + user swatch) — confirmed default. Worth introducing a per-kind palette on web as an enhancement, or keep strict parity?
3. **Icon set on web.** Use the Material Symbols web font (matching `ActivityKindVisuals.kt` glyph names) or a bundled inline SVG set? Either way keep names aligned with Android.
4. **Track kind UI placement.** Where the optional track kind picker lives in the track create/edit flow (`08`/`09`).
