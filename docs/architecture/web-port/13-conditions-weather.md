# 13 — Conditions / weather

> Surface point weather, multi-day forecast, avalanche danger, marine
> conditions + tides, and sun-driven lighting wherever the user is looking on
> the map — and feed live wave/wind into the realistic-water renderer.

## Status
- **Android (gold standard):** A `ConditionsRepository.forPoint(lat,lng)` aggregates four external sources into one `Conditions` object and surfaces it across the app:
  - **Weather now** — MET `locationforecast/2.0/compact`. `WeatherNow { temperatureC, windSpeedMs, windFromDeg, precipitationMm, symbolCode, humidityPct, cloudCoverPct, uvIndex }`. Shown as a mini-card in the long-press menu, a row in marker detail, and the header of the conditions sheet.
  - **Forecast** — same MET timeseries parsed into `WeatherForecast { points: List<AtmosphericPoint>, days: List<DailySummary> }`. Hourly `AtmosphericPoint { timeIso, temperatureC, windSpeedMs, windFromDeg, humidityPct, cloudCoverPct, uvIndex, precipitation1hMm, symbol1h }`; daily rollup `DailySummary { date, minTempC, maxTempC, totalPrecipMm, middaySymbol }` (~7 days). Rendered in `WeatherForecastSheet` (day chips + hourly rows).
  - **Avalanche** — Varsom/NVE `AvalancheWarningByCoordinates`. `AvalancheNow { dangerLevel(1–5), mainText, region, problems }`; `AvalancheProblem { type, trigger, distribution, size }`. Visibility rule `shouldShowAvalanche()`: level ≥ 3 always; level 2 only when air temp ≤ 5 °C; level ≤ 1 never.
  - **Marine** — MET `oceanforecast/2.0/complete` → `MarineNow { waveHeightM, waveFromDeg, seaTemperatureC, seaCurrentSpeedMs, hasData }`; Kartverket `tideapi.php` → `TideForecast { stationName, extrema }`, `TideExtreme { timeIso, levelCm, kind: High|Low }`. Shown in an **Ocean** tab in the conditions sheet (only when `hasData`), plus a one-line `MarineRow` in marker detail.
  - **Sun/lighting** — `SunOverlayControls` scrubs an hour-of-day slider; the wgpu engine (`TerrainSunOverlay.setSunTime`, `setTerrainShadows`) solves solar azimuth/altitude and drives sky colour + cast shadows.
  - **Water feed** — `MapViewModel` polls conditions for the camera centre (rounded to a ~2 km grid), publishes a `SeaState`, and `MapScreen` casts the engine to `WaterConditionsOverlay.setWaterConditions(waveFromDeg, waveHeightM, windSpeedMs, windFromDeg)` to animate the live water surface.
  - Entry points: **marker detail**, **route detail** (`RouteConditionsStrip` samples ~4 points along the geometry → min/max temp + worst avalanche level), **long-press map → weather menu**, and a **standalone conditions sheet**.
  - All external calls are made **directly** from the app over Ktor with `User-Agent: turbo-expressive/0.1 github.com/SigmundGranaas/turbo` and HTTP caching. There is **no backend proxy** today.
- **Web today:** Not implemented. No weather, avalanche, marine, or conditions UI exists.
- **Renderer/back-end prerequisites:**
  - **Expose `set_water_conditions` in `turbomap-web`** (thin wasm-bindgen passthrough over the engine's existing `WaterConditionsOverlay`; cross-link [03 — Map realism](03-map-realism.md)). `set_sun_time` + `set_terrain_shadows` already listed in the web wrapper.
  - **CORS / User-Agent open question (see below):** browsers cannot set a custom `User-Agent`, and MET *requires* an identifying UA. The likely answer is a **backend proxy** (`/api/conditions/*` or per-source proxy routes) that adds the UA and forwards to MET / Varsom / Kartverket. Kartverket's tide API returns XML, so the proxy can also normalize to JSON.

## User stories

### 1. See current weather at a point
*As a hiker, I want to see the current temperature, wind, precipitation, humidity, cloud cover and UV at a tapped/long-pressed point, so that I can judge conditions before I set out.*

**Acceptance criteria**
- Long-pressing the map opens a weather popover showing the MET symbol icon, temperature (°C), wind speed (m/s) + direction arrow (`windFromDeg`), and next-hour precipitation (mm/h).
- Opening a marker's detail shows the same current-weather row for the marker's coordinate.
- Wind direction is rendered as a compass arrow pointing *from* `windFromDeg`.
- Units follow the user's settings (see [19 — Settings](19-settings.md)); default metric (°C, m/s, mm).
- Tapping the popover/row opens the full conditions sheet (story 2).
- While fetching, a loading skeleton shows; the result is cached so re-opening the same point is instant.

**Web-specific notes:** request is keyed by coordinate rounded to ~4 decimals (matching Android) to hit the TanStack Query cache and respect MET's caching. Browser cannot set `User-Agent` → request goes through the backend proxy (open question).

### 2. See a multi-day forecast at a point
*As a trip planner, I want a 7-day forecast with hourly detail and daily summaries, so that I can pick the best day.*

**Acceptance criteria**
- The conditions sheet **Weather** tab shows a horizontal day strip: per day a weekday/date chip, the midday symbol, and max/min temp (`DailySummary`).
- Selecting a day shows that day's hourly rows: time, symbol, temp, precip (mm/h), wind.
- Days come from the daily rollup derived from the MET timeseries (~7 days); hours come from `AtmosphericPoint`s for the selected day only.
- The tail of the timeseries may have `null` `symbol1h`/`precipitation1hMm`; rows degrade gracefully (omit the missing chip, never show "NaN").
- The sheet header keeps the current-weather summary visible.

**Web-specific notes:** day/hour grouping is done client-side from the MET timeseries (Android does this in `WeatherSummary.dailySummaries`). MET symbol icons are bundled SVGs keyed by `symbolCode` (e.g. `partlycloudy_day`), with a `cloudy` fallback for unknown codes — reuse the same icon set as Android/Flutter.

### 3. Check avalanche danger
*As a backcountry skier, I want to see avalanche danger for a location when it's relevant, so that I'm warned without being spammed in safe/summer conditions.*

**Acceptance criteria**
- An avalanche card appears **only** when `dangerLevel ≥ 3`, **or** `dangerLevel == 2` and the point's air temp ≤ 5 °C; otherwise it is hidden entirely.
- The card shows a colour-coded 1–5 danger badge, the region name, the main bulletin text, and a bulleted `problems` list (`type · trigger · size`, omitting null fields).
- In the long-press menu and route strip, only the danger-level badge is shown (compact); the full text lives in the conditions sheet / marker detail.
- If NVE returns no warning for the coordinate (outside a forecast region), the card is hidden, not an error.

**Web-specific notes:** Varsom/NVE has no UA requirement but is still cross-origin → proxy or confirm CORS. Date defaults to today (Android queries `today/today`).

### 4. Check marine conditions + tides at the coast
*As a sea kayaker, I want wave height/direction, sea temperature, sea current, and the next high/low tides, so that I can plan a safe coastal outing.*

**Acceptance criteria**
- The conditions sheet shows an **Ocean** tab **only when** marine data exists (`MarineNow.hasData` or tide extrema present) — inland points show no Ocean tab.
- Ocean tab tiles: wave height (m) + direction (compass arrow from `waveFromDeg`), sea temperature (°C), and sea current speed (m/s) when present.
- A tide table lists upcoming high/low extrema: station name, kind (High/Low), local time, level (cm above chart datum). When a forecast day is selected, the table shows that day's extrema, falling back to all extrema.
- Marker detail shows a compact `MarineRow` ("Sea, 12° water · 1.5 m waves" + wave-direction arrow) when marine data exists.
- If only one of MET-ocean / Kartverket-tide returns data, render the part that's present.

**Web-specific notes:** Kartverket `tideapi.php` returns **XML** (`<waterlevel time value flag="high|low"/>`); the proxy should parse it to JSON (params Android uses: `datatype=tab`, `refcode=cd`, `lang=en`, `dst=1`, `tide_request=locationdata`, 6 h past → 3 d future window). Sea current is often absent in MET's response → treat as optional.

### 5. See weather along a route
*As a route planner, I want a conditions summary sampled along my planned route, so that I know what to expect across the whole trip, not just the start.*

**Acceptance criteria**
- A route's detail card shows a conditions strip: min/max temperature across ~4 points sampled along the geometry, and the worst avalanche danger level encountered (if any sample is ≥ 1).
- The strip is compact (no per-point hourly detail) and links to the full conditions sheet for any sampled point.
- Sampling and fetches are debounced/cached so editing the route doesn't fire a request per keystroke.

**Web-specific notes:** sampling matches Android (`RouteConditionsStrip`, ~4 points). Fan-out of 4 point queries reuses the same TanStack Query cache as single-point lookups (coordinate-rounded keys), so overlapping requests dedupe. Cross-link [09 — Routing](09-routing.md).

### 6. Sun position drives 3D lighting
*As a user in 3D mode, I want the terrain lit by the real sun for a chosen time of day, so that the scene looks physically plausible and I can preview light/shadow.*

**Acceptance criteria**
- In 3D mode, an hour-of-day slider scrubs the sun across today's date; the engine updates sky colour, terrain shading, and cast-shadow opacity in real time.
- The slider only appears when the WebGPU engine is active (always true on web — no MapLibre fallback).
- Closing the control leaves the last-set time in effect.

**Web-specific notes:** uses the already-listed `set_sun_time(unixSeconds)` and `set_terrain_shadows(strength)` web methods. Cross-link [03 — Map realism](03-map-realism.md) and [02 — 3D camera + terrain](02-3d-camera-terrain.md). Solar position is computed in-engine from epoch seconds + camera lat/lng.

### 7. Live wave/wind feeds the water renderer
*As a user near the coast, I want the rendered water to reflect the actual sea state, so that the realistic-water surface matches conditions.*

**Acceptance criteria**
- When marine + wind data is available for the camera centre, the water surface animates with the real `waveFromDeg`, `waveHeightM`, `windSpeedMs`, `windFromDeg`.
- The feed is throttled: the camera centre is rounded to a coarse (~2 km) grid so panning doesn't hammer MET.
- When no marine data exists (inland / API failure), the water falls back to its default animated state (no error surfaced to the user).

**Web-specific notes:** **requires exposing `set_water_conditions` in `turbomap-web`** (passthrough over the engine's `WaterConditionsOverlay`). Cross-link [03 — Map realism](03-map-realism.md). The same `Conditions` fetch that powers the conditions sheet supplies these values — one query, two consumers (UI + renderer).

## Primary flows (web)

**Happy path — long-press → weather:** user long-presses the map → a weather popover anchors at the point and a coordinate-keyed conditions query fires (via proxy) → popover fills with symbol/temp/wind/precip; if avalanche is relevant a compact danger badge appears → tapping "More" opens the conditions bottom sheet with Weather / Ocean (if marine) / Avalanche tabs.

**Marker detail:** opening a marker (see [06 — Markers](06-markers.md)) renders a current-weather row + (conditional) avalanche card + (conditional) marine row, each linking into the conditions sheet for that coordinate.

**Route conditions:** after a route resolves (see [09 — Routing](09-routing.md)), ~4 points are sampled and fetched; the route card shows min/max temp + worst avalanche level.

**Empty / inland:** inland points show Weather only — no Ocean tab, no marine row; avalanche hidden unless the rule fires. The sheet still opens; absent sections are simply not rendered.

**Missing data:** individual fields render only when non-null (e.g. UV/sea-current often absent); never show placeholders or NaN. A day with no symbol shows temp/precip only.

**API failure / offline:** if the proxy/source errors, the conditions area shows a small inline "Conditions unavailable — retry" state (not a blocking error). The water renderer keeps its default state. A retry re-runs the query.

**Unauthenticated:** weather/avalanche/marine are public data and do **not** require login — they work for anonymous users (only marker/route *attachment* points come from authed features).

## UI / UX on web

- **Conditions bottom sheet** (mobile) / right-side panel (desktop) with **Weather / Ocean / Avalanche** tabs. Ocean and Avalanche tabs are present only when their data is relevant. When the sheet is open on mobile, set `set_viewport_inset` so the map centre isn't occluded.
- **Long-press weather popover:** a lightweight floating card anchored to the pressed point (desktop pointer) / above the touch point (touch). Contains the current-weather summary + a "More" affordance → opens the sheet.
- **Marker detail integration:** weather row + conditional avalanche card + conditional marine row inside the existing detail panel.
- **Route card integration:** a single-line conditions strip beneath the route metrics.
- **Sun control:** a slider in the 3D mode controls overlay (only when WebGPU 3D is active).
- Responsive: on desktop the conditions live in a side panel (no viewport inset needed); on touch they live in a draggable bottom sheet.

## Data & APIs

External sources (all currently called directly by Android; on web **likely proxied via backend** — open question):

| Source | URL | Key params | Returns |
|---|---|---|---|
| MET locationforecast | `https://api.met.no/weatherapi/locationforecast/2.0/compact` | `lat`, `lon` (4 dp) | timeseries → `WeatherNow` + `WeatherForecast` |
| MET oceanforecast | `https://api.met.no/weatherapi/oceanforecast/2.0/complete` | `lat`, `lon` | `MarineNow` |
| Varsom/NVE avalanche | `https://api01.nve.no/hydrology/forecast/avalanche/v6.2.1/api/AvalancheWarningByCoordinates/Detail/{lat}/{lng}/1/{today}/{today}` | path coords + date | `AvalancheNow[]` |
| Kartverket tide | `https://vannstand.kartverket.no/tideapi.php` | `lat`, `lon`, `fromtime`, `totime`, `datatype=tab`, `refcode=cd`, `lang=en`, `dst=1`, `tide_request=locationdata` | XML → `TideForecast` |

- **Auth:** none — all conditions data is public. No cookie/token required.
- **MET requires** `User-Agent: turbo-expressive/0.1 github.com/SigmundGranaas/turbo` (or equivalent identifying UA). Browsers cannot set this header → drives the proxy decision.
- **TanStack Query keys:**
  - `['weather', latR, lonR]` → current + forecast (one MET fetch covers both).
  - `['avalanche', latR, lonR, dateISO]`.
  - `['marine', latR, lonR]` and `['tide', latR, lonR]` (or a combined `['ocean', …]`).
  - `latR/lonR` = coordinate rounded to ~4 dp; route sampling reuses these keys.
  - Stale times tuned to MET caching (e.g. weather ~10 min, tide/avalanche longer).
- **Zustand:** a small `conditionsUi` slice for sheet open state + active tab + selected forecast day + sun-time; a `seaState` value (or derived selector) drives the water feed.

## Renderer integration

- **Sun/lighting:** `set_sun_time(unixSeconds)` + `set_terrain_shadows(strength)` (already in the web wrapper) — fed by the sun slider.
- **Water:** `set_water_conditions(waveFromDeg, waveHeightM, windSpeedMs, windFromDeg)` — **must be added to `turbomap-web`** as a wasm-bindgen passthrough over the engine's existing `WaterConditionsOverlay`. Fed by the throttled marine/wind values for the camera centre. See [03 — Map realism](03-map-realism.md).
- No new Scene sources/layers are required for conditions UI itself (weather is panel data, not a map layer). The avalanche/wave/wind *overlay layers* are a separate concern — see [04 — Vector overlays](04-vector-overlays.md).

## Out of scope (this phase)
- Offline cached conditions / last-known-good persistence (offline phase).
- Push/notification alerts for danger changes.
- Avalanche-region polygon overlay on the map (covered by [04 — Vector overlays](04-vector-overlays.md)).
- Historical/observed conditions logging (see [14 — Activities](14-activities.md)).

## Open questions
- **CORS + User-Agent:** MET *requires* an identifying `User-Agent` that the browser cannot set, and these are third-party origins. Confirm the resolution: a backend proxy (`/api/conditions/*` adding the UA, normalizing Kartverket XML → JSON, and optionally caching) is the expected answer — **flagged as the likely required approach.** Decide whether to proxy per-source or expose one aggregated `forPoint` endpoint mirroring Android's `ConditionsRepository`.
- Should the proxy aggregate (one `forPoint` call → all four sources) or stay thin per-source? Aggregation matches Android's repository shape and reduces round-trips; thin proxying is simpler to cache per source.
- Confirm the exact bundled MET symbol icon set is shared with Android/Flutter (and the `cloudy` fallback).
- Tide window / units: keep Android's 6 h-past→3 d-future window and cm-above-chart-datum, or surface a user-facing units choice?
