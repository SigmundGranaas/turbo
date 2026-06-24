# 12 — Search: coordinates, markers, places, trails, recents

> Let a web user search one box that fuses coordinate parsing, their own markers, Kartverket place names, and Nasjonalturbase trails in real time, filter the results, reuse recents, and fly the camera to any pick.

## Status
- **Android (gold standard):** A single search screen fuses **four sources** in
  real time (`SearchViewModel`):
  - **Coordinate parse** — instant, synchronous; regex
    `^\s*(-?\d{1,3}(?:\.\d+)?)\s*[,;\s]\s*(-?\d{1,3}(?:\.\d+)?)\s*$` (comma /
    semicolon / space separated), validated lat ∈ [-90, 90], lng ∈ [-180, 180];
    yields a `Coordinate` result. **Always shown** regardless of filter.
  - **Local markers** — instant, case-insensitive substring
    (`marker.name.contains(query, ignoreCase = true)`); yields `Marker` results
    with the marker's activity kind.
  - **Kartverket place names** — network, **debounced `DEBOUNCE_MS = 280`**
    (`SearchRepository` → `api.kartverket.no/stedsnavn/v1/navn`, `fuzzy=true`,
    `treffPerSide=8`); yields `Place` results.
  - **Nasjonalturbase named trails** — network, debounced
    (`TrailSearchRepository` → `wfs.geonorge.no` `friluftsruter2` WFS,
    `navn ILIKE '%q%'`, `COUNT=10`); yields `Trail` results.
  The two network sources fan out **concurrently**; instant results publish
  immediately with `loading = true`, then network results merge. **Filter chips**
  are **All / Markers / Places** (Markers = coords + markers; Places = coords +
  places + trails). **Recents** (`RecentSearchRepository`, DataStore, `MAX = 8`,
  case-insensitive + 4-decimal-position dedup, most-recent-first) show when the
  query is empty, with a **Clear** action. Tapping a result **flies the camera to
  zoom 14** and **records it to recents**. An **error is shown only if BOTH
  network sources fail**; a single source failure still surfaces the other's
  results.
- **Web today:** Not implemented. No search box, no places client, no recents.
- **Renderer/back-end prerequisites:**
  - **No renderer change.** Search only drives the camera (`ease_to` / `flyTo`).
  - **Places (public):** `GET /api/places/search?q=&lat=&lon=&limit=` (the app's
    proxy over Kartverket stedsnavn) — preferred over hitting Kartverket directly
    from the browser (CORS + a single backend contract). **Trails source TBD**:
    either a backend trails endpoint or a proxied Nasjonalturbase WFS call (see
    Open questions — the public WFS may not send CORS headers).
  - **Markers:** local markers from the markers query/store (doc 06).

## User stories

### 1. Search by coordinate
*As a user with a coordinate, I want to type `lat,lng` (or `lat lng`) and jump there instantly, so that I can go to a known point without network.*

**Acceptance criteria**
- Typing a string matching the coordinate regex (comma, semicolon, or
  whitespace separated, optional decimals, optional leading `-`) yields an
  **instant** Coordinate result at the top, with no network round-trip.
- Values out of range (lat > 90 / lng > 180) do **not** produce a coordinate
  result (the text falls through to the other sources).
- The coordinate result is shown under **every** filter chip (All / Markers /
  Places).
- Picking it flies the camera to that point at zoom 14 and records it to recents.

**Web-specific notes**
- Port the regex verbatim. Parse synchronously on each keystroke before any
  debounce.

### 2. Search my markers
*As a user, I want to find my own saved markers by name as I type, so that I can return to a place I pinned.*

**Acceptance criteria**
- Marker matching is **instant**, **case-insensitive substring** over the marker
  name; results show the marker name + a subtitle (e.g. "Saved marker · {kind}").
- Marker results appear under **All** and **Markers** chips, not under **Places**.
- Works fully **offline of the network sources** (uses the already-loaded markers
  store) — markers still match even if the network sources fail.

**Web-specific notes**
- Read from the doc 06 markers query/store already in memory; no extra fetch.

### 3. Search places and trails
*As a user planning a trip, I want Kartverket place names and named trails as I type, so that I can find a mountain, lake, town, or marked trail by name.*

**Acceptance criteria**
- Place + trail queries are **debounced ~280 ms** and only fire for non-empty
  queries; the two sources run **concurrently**.
- While network is in flight, instant results (coords + markers) are already
  shown with a loading indicator; place/trail results merge in when they arrive.
- Place results show name + descriptor (type + municipality); trail results show
  name + a trail descriptor. Results carry a position so they're tappable.
- Place + trail results appear under **All** and **Places** chips, not **Markers**.

**Web-specific notes**
- Places via `GET /api/places/search?q=&lat=&lon=&limit=` (pass current map
  center as `lat`/`lon` to bias ranking, `limit≈8`). Trails via the TBD trails
  source. Both are public (no auth).

### 4. Filter results
*As a user, I want to narrow results to just markers or just places, so that I can cut through noise when I know what I'm after.*

**Acceptance criteria**
- Three chips: **All** (default), **Markers**, **Places**.
- **All** shows coords + markers + places + trails; **Markers** shows coords +
  markers; **Places** shows coords + places + trails.
- The **coordinate** result is always shown regardless of the selected chip.
- Switching chips re-filters the **already-fetched** results instantly (no
  refetch).

### 5. Use recents
*As a returning user, I want my recent searches when the box is empty, so that I can jump back to recent places quickly.*

**Acceptance criteria**
- With an **empty query**, the list shows recents **most-recent-first**, capped at
  **8**, with a **Clear recents** action.
- Picking a result records it to recents; duplicates are collapsed
  (case-insensitive name + position rounded to ~4 decimals) and re-promoted to the
  top.
- Recents persist across reloads on the same browser.

**Web-specific notes**
- Persist recents in **`localStorage`** (web has no DataStore), same dedup/cap
  rules as `RecentSearchRepository`.

### 6. Jump to a result
*As a user, I want tapping any result to fly the map there, so that selecting is the whole action.*

**Acceptance criteria**
- Tapping a result with a position eases/flies the camera to it at **zoom 14** and
  closes the search overlay (or collapses it on desktop).
- The pick is recorded to recents (story 5).
- Results without a resolvable position (shouldn't normally happen) are non-tappable
  rather than flying to `(0,0)`.

**Web-specific notes**
- Camera move via `ease_to`/`set_camera` to `(lat, lng, 14)`.

### 7. Empty, no-result, and failure states
*As a user, I want clear, non-alarming feedback when there's nothing to show, so that I'm not confused by a blank box.*

**Acceptance criteria**
- **Empty query** → recents (or a hint if none).
- **No results** for a non-empty query (after network settles) → a quiet "No
  results" message, not an error.
- **One network source fails** but the other succeeds → results still show; **no
  error** surfaced.
- **Both network sources fail** → a single inline error ("Couldn't reach search —
  check your connection.") with **Retry**; instant results (coords/markers) are
  still shown above it.
- **Stale requests are cancelled**: a newer keystroke supersedes an in-flight
  request so out-of-order responses never overwrite fresher results.

**Web-specific notes**
- Use `AbortController` per network request, aborting the previous one on each new
  debounced query — this is the web analog of Android's coroutine cancellation
  and guarantees the stale-response acceptance criterion.

## Primary flows (web)

**Happy path — type, filter, pick**
1. User focuses the top-bar search box → recents (most-recent-first) appear.
2. User types `gaust`. Synchronous: coordinate regex fails (not a coord); local
   markers filtered instantly. Results render with a loading spinner.
3. After 280 ms idle, places + trails fetch concurrently (`AbortController` per
   request, current map center passed as bias). "Gaustatoppen" (place) +
   "Gaustaløype" (trail) merge in.
4. User taps **Places** chip → marker rows hide; place/trail (+ any coord) remain,
   instantly, no refetch.
5. User taps "Gaustatoppen" → camera eases to it at zoom 14; overlay closes; the
   pick is prepended to recents (deduped).

**Edge — coordinate**
1. User types `59.93, 10.75` → instant Coordinate result on top under any chip →
   pick flies to zoom 14, recorded.

**Edge — one source down**
- Trails WFS times out but places returns: place results show, no error.

**Edge — both sources down**
- Inline error + Retry; coords/markers still shown.

**Edge — empty / no results**
- Empty box → recents. `"asdfgh"` with no hits → "No results".

**Edge — fast typing**
- Each keystroke aborts the prior in-flight fetch; only the latest query's results
  land.

## UI / UX on web
- **Where:** a **search box in the top bar** (or a search affordance that expands
  to an overlay panel). On narrow/touch it expands to a full-width overlay with a
  results list; on wide/pointer it's a dropdown panel anchored to the box.
- **Composition with canvas:** search overlays the map; it does not need
  `set_viewport_inset` (it's a transient panel, not a persistent sheet). Closing
  returns focus to the map.
- **Chips:** All / Markers / Places below the input; the active chip is visually
  selected.
- **Keyboard:** Esc closes; ↑/↓ move the result highlight; Enter picks the
  highlighted result (or the top result). The box is the focus target.
- **State:** a Zustand `searchStore` (query, filter chip, results, loading, error)
  + TanStack Query for the network sources keyed by debounced query.

## Data & APIs
- **Places (public):** `GET /api/places/search?q=&lat=&lon=&limit=` — `q` =
  query, `lat`/`lon` = current map center (ranking bias), `limit ≈ 8`. No auth.
- **Trails (public, TBD):** a backend trails endpoint or proxied Nasjonalturbase
  WFS (`friluftsruter2`, `navn ILIKE '%q%'`, `COUNT=10`) — see Open questions.
- **Markers:** from the doc 06 markers query/store (in memory; auth-scoped data
  but the matching is local).
- **Recents:** `localStorage` (key e.g. `turbo.search.recents`), `MAX = 8`,
  case-insensitive + 4-decimal-position dedup, most-recent-first.
- **State:** Zustand `searchStore`; TanStack Query keys `['places', q, center]`
  and `['trails', q]`, each with an `AbortController` (cancel-stale).

## Renderer integration
- **Scene sources/layers:** none — search does not add layers.
- **turbomap-web methods:** `ease_to` / `set_camera` to fly to a pick at zoom 14.
- **No new passthrough required.**

## Out of scope (this phase)
- Offline place/trail search (no cached gazetteer) — network sources require
  connectivity; coords + markers still work offline-of-network.
- Reverse geocoding / "what's here" (that's `/api/places/reverse`, a separate
  feature).
- Search history sync across devices (recents are per-browser `localStorage`).
- Rich result previews (thumbnails, elevation) beyond name + subtitle.

## Open questions
- **Trails source on web:** does the backend expose a trails search endpoint, or
  must the browser hit Nasjonalturbase WFS directly? The public WFS likely lacks
  CORS headers → we probably need a **proxied/backend trails endpoint**. Confirm
  the contract.
- Should `/api/places/search` results include enough to render the **subtitle**
  (type + municipality) that Android shows, or is a second lookup needed?
- **Debounce**: keep 280 ms to match Android, or tune for web network latency?
- Should recents be **synced** when authenticated (account settings) or strictly
  per-browser like Android's per-device DataStore?
- Result **ranking/merge order** across sources when "All" is selected (Android's
  fused order) — confirm the intended priority (coords → markers → places →
  trails?).
