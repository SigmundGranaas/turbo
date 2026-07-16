# Feature matrix: Flutter (final) vs Android vs Web

**Date:** 2026-07-16.
**Scope:** the three phone/desktop clients. Flutter (`apps/flutter`) was **removed on 2026-07-05**
(`4b25ebb1`, "P5.3 — remove the Flutter app (web + Android are the product)"); its column reflects
the app's **final state** at `4b25ebb1^` and serves as the migration baseline. Android
(`apps/android`, Compose + wgpu turbomap over JNI) and Web (`apps/web`, React + wgpu turbomap over
WASM) are the two product clients. iOS (`apps/ios`, MapKit) exists but is outside this analysis.

**Legend:** ✅ complete · 🟡 partial/stub · ❌ absent · — not applicable by design.

---

## 1. Map & rendering

| Feature | Flutter (final) | Android | Web | Notes |
|---|:-:|:-:|:-:|---|
| Map engine | ✅ flutter_map, 2D raster/vector | ✅ wgpu native (JNI) | ✅ wgpu WASM (WebGPU-only, no WebGL2 fallback) | Successors share one Rust engine |
| Base layers | ✅ Norgeskart, Turbo N50, OSM, Google sat, **UTM33 high-detail** | ✅ Norgeskart, OSM, satellite | ✅ Kartverket, OpenTopoMap, OSM, Google sat | Flutter's EPSG:25833 high-detail mode has no successor |
| Custom user tile URLs | ✅ | ❌ | ❌ | **Lost in migration** |
| 3D terrain (DEM displacement) | ❌ (2D only) | ✅ | ✅ | New capability, never in Flutter |
| Hillshade / sun-lit relief | ❌ | ✅ | ✅ | |
| Sun mode: movable sun + cast terrain shadows | ❌ (sunrise/sunset data only) | ✅ time-of-day slider | ✅ (forces 3D) | |
| Atmosphere / distance haze | ❌ | ✅ | 🟡 opt-in haze; sky not user-controllable | |
| Clouds / radar on the map | ❌ | ✅ procedural clouds from MET radar, scrubbable | ❌ (weather is panel-only) | |
| Route/track as raised 3D tubes | — | ✅ | ❌ (draped lines) | |
| Data overlays | ✅ slope-angle, avalanche, ocean, Nasjonal Turbase, curated MVT trails, external vector layers | 🟡 Trails + avalanche/bratthet; Waves/Wind enum stubs unwired | ❌ none | **Biggest rendering gap vs Flutter** |
| Layer switching + persisted prefs | ✅ | ✅ | ✅ | |
| Compass reset / bearing | ✅ | ✅ | ✅ live needle | |
| Scale bar | ✅ | ✅ | 🟡 `MapReadout` is a hardcoded visual stub | Web readout not wired to camera |
| Long-press point menu (elevation, weather, marker, route here) | ✅ coordinate detail | ✅ | ✅ terrain-raycast context menu | |

## 2. Track recording

| Feature | Flutter (final) | Android | Web | Notes |
|---|:-:|:-:|:-:|---|
| GPS recording | ✅ | ✅ foreground service | — | Web has locate-me only; recording out of scope in a browser |
| Pause / resume | ✅ (gap excluded from moving time) | ✅ + **buffer-while-paused + Include/Discard prompt** (US-4) | — | |
| Live stats | 🟡 distance, ±elevation, moving time, speed — **no kcal** | ✅ + kcal, max speed, altitude | — | Android altitude now MSL-corrected (8e1d8bc0) |
| Draft / crash recovery | ❌ | ✅ DataStore draft, survives process death | — | |
| Lock-screen presence | 🟡 basic foreground notification | ✅ Live Update notification with pause/resume/stop + Quick Settings tile | — | |
| Live sheet UI | ✅ outing panel | ✅ draggable sheet, hero stats, elevation spark | — | |

## 3. Route planning

| Feature | Flutter (final) | Android | Web | Notes |
|---|:-:|:-:|:-:|---|
| Waypoints add / move / remove | ✅ | ✅ (drag fixed 8e1d8bc0) | ✅ | |
| Insert stop at least-detour position | ✅ | ✅ | ❌ | |
| Undo | ✅ undo-last | ✅ 20-deep stack | ❌ | |
| Route styles / presets | ✅ | ✅ | ✅ + **profiles (Hike/Ski/Bike)** | Profiles are web-only so far |
| Streaming solve (SSE progress) | ✅ | ✅ | ✅ | All against `/api/route/plan/stream` |
| Graceful re-solve (keep old line) | — | ✅ | ✅ dashed preview | |
| Elevation profile in planner | 🟡 only after saving | ✅ RouteCard | ❌ (saved tracks only) | |
| Save route as track | ✅ | ✅ (+ activity kind) | ✅ | |
| Line (straight segments) mode | 🟡 via measuring tool | ✅ | ❌ | |
| Freehand Draw mode | ✅ (drawing settings) | ✅ | ❌ | |

## 4. Following / navigation

| Feature | Flutter (final) | Android | Web | Notes |
|---|:-:|:-:|:-:|---|
| Follow a route / saved track | ✅ unified "journey" | ✅ | ❌ explicitly out of scope | |
| Progress cursor, remaining, ETA | ✅ (global-nearest) | ✅ arc-length monotonic cursor (loop-safe) | ❌ | Android cursor fixes Flutter's out-and-back false "Arrived" |
| Checkpoints / phase split times | ❌ | ✅ stops + nearby markers, on-map pins | ❌ | |
| Off-route reroute | 🟡 banner at 50 m, manual geometry swap | ✅ silent auto re-solve | ❌ | |
| Follow = Record (capture real track) | 🟡 journey unified state, no auto-save | ✅ + auto-save with trivial-session guard | ❌ | |
| Dim already-covered guide | ❌ | ✅ | ❌ | |
| Arrival handling | ✅ 30 m auto-finish latch | ✅ stop-save prompt, lock-screen sync | ❌ | |

## 5. Saved tracks / paths

| Feature | Flutter (final) | Android | Web | Notes |
|---|:-:|:-:|:-:|---|
| List / detail / stats | ✅ + trip-stats page | ✅ | ✅ + search & sort | |
| Elevation chart | ✅ + **elevation backfill (høydedata)** | ✅ | ✅ | Backfill lost in migration |
| Import | ✅ GPX/KML/GeoJSON | ✅ GPX/KML/GeoJSON auto-detect | ✅ GPX/KML/GeoJSON (±3 m ascent hysteresis, 684d1fa5) | |
| Export | ✅ GPX/GeoJSON | ✅ GPX/KML share | ✅ GPX/KML/GeoJSON download | |
| Line style customization | ✅ solid/dot/dash + colour | ❌ rename/delete only | 🟡 colour/icon metadata | **Lost on Android** |
| Activity kinds | ✅ (activity subsystem) | ✅ 18 kinds | ✅ 18 kinds | |
| Sync | ✅ local-first + sync service | ✅ pull→merge→push cursors | ✅ online-first (no local store) | |

## 6. Markers / POIs

| Feature | Flutter (final) | Android | Web | Notes |
|---|:-:|:-:|:-:|---|
| Create / edit / delete | ✅ | ✅ | ✅ (+ reverse-geocoded name prefill) | |
| Icons / kinds / colours | ✅ icon catalog + picker | ✅ 18 kinds + per-marker colour | 🟡 kind icons, client-side tint only | |
| Notes | ✅ | ✅ | ✅ | |
| Photos on markers | ✅ | ✅ | ❌ | |
| Collections membership | ✅ | ✅ | ✅ | |
| Marker export | ✅ | ❌ | ❌ | **Lost in migration** |

## 7. Photos

| Feature | Flutter (final) | Android | Web | Notes |
|---|:-:|:-:|:-:|---|
| Camera capture (geotagged) | ❌ | ✅ | ❌ | |
| Device photo-library scan → map layer | ✅ (photo_manager, per-asset GPS) | 🟡 gallery *import* only, no library scan | ❌ | Different models: Flutter surfaces the whole library, Android curates |
| On-map photo pins + clustering | ✅ thumbnails | ✅ | ❌ | |
| Gallery / viewer | ✅ | ✅ | ❌ | |

## 8. Offline maps

| Feature | Flutter (final) | Android | Web | Notes |
|---|:-:|:-:|:-:|---|
| Region download | ✅ orchestrator/worker/queue | ✅ viewport download + size estimate | — online-first by design | |
| Region management | ✅ | ✅ rename/pause/resume/retry/delete+undo | — | |
| Corridor download along route | ✅ 1 km buffer | ✅ | — | |
| Download policy | ❌ | ✅ Wi-Fi-only toggle | — | |
| Storage stats / cache | ✅ stats page | 🟡 clear-cache + coverage indicator | — | |

## 9. Search

| Feature | Flutter (final) | Android | Web | Notes |
|---|:-:|:-:|:-:|---|
| Place names | ✅ | ✅ | ✅ | |
| Coordinate parse | ✅ | ✅ | ✅ | |
| Reverse geocode | ✅ | ✅ | ✅ | |
| Multi-backend breadth | ✅ address, kommune, protected areas, elevation, markers, trails | 🟡 places, markers, trails | 🟡 places + coordinates only | **Address & kommune search lost** |
| Recent searches | ❌ | ✅ | ❌ | |
| Filter tabs | ❌ | ✅ All/Markers/Places | ❌ | |

## 10. Weather / conditions

| Feature | Flutter (final) | Android | Web | Notes |
|---|:-:|:-:|:-:|---|
| Point forecast (MET) | ✅ | ✅ hourly + daily | ✅ now/24 h/daily | |
| Ocean / tides | ✅ | ✅ marine + tide extrema | ✅ tide + summary | |
| Route-along conditions | ❌ | ✅ strip along solved route | ❌ | Android-only |
| Precipitation radar | ❌ | ✅ map overlay (with playback) | ❌ numeric only | |
| MET weather alerts | ✅ banner | ❌ | ❌ | **Lost in migration** |
| Avalanche forecast (Varsom) | ✅ warning badge + sheet | 🟡 bratthet/slope overlay only | ❌ deferred (needs region resolver) | **Forecast lost; only slope remains** |
| Sun events (sunrise/sunset) | ✅ | 🟡 via sun-mode clock, not surfaced as data | ❌ | |

## 11. Auth, sync & sharing

| Feature | Flutter (final) | Android | Web | Notes |
|---|:-:|:-:|:-:|---|
| Email/password + Google OAuth | ✅ | ✅ Custom Tab + Keystore tokens | ✅ cookie session | |
| Profile edit / change password | ✅ | 🟡 account screen; Settings header shows a hardcoded name stub | 🟡 profile panel | |
| Data sync (tracks/markers/collections) | ✅ local-first | ✅ cursor-based engine + toggle | ✅ online-first (TanStack Query, If-Match) | |
| Share links (create/redeem, roles) | ✅ | ✅ | ✅ | |
| Friend code | ✅ | ✅ | ✅ | |
| Friends (request/accept) | ✅ | 🟡 client scaffolding; repo methods return "unsupported" | ❌ | Backend not wired for successors |
| Groups | ✅ | 🟡 same scaffolding state | ❌ | |
| Visibility levels (private/friends/unlisted/public) | ✅ | 🟡 via share roles | 🟡 via share links | |

## 12. Settings / misc

| Feature | Flutter (final) | Android | Web | Notes |
|---|:-:|:-:|:-:|---|
| Theme (system/light/dark) | ✅ | ✅ | ✅ | |
| Units (metric/imperial) | ✅ | ✅ | ✅ | |
| Camera persistence across launches | ❌ | ✅ | ✅ full pose incl. pitch/bearing | Successors improved on Flutter |
| Location-marker / heading customization | ✅ colour + icon picker | ❌ | ❌ | Lost |
| Recording / drawing preference pages | ✅ | ❌ (defaults only) | — | |
| Permission flows | ✅ fg/bg dialogs | ✅ | — | |
| Desktop layout | 🟡 desktop search bar variants | — | ✅ NavRail + side panels + viewport inset | |
| Keyboard shortcuts | ❌ | — | ✅ pan/orbit/zoom/Escape | |
| Collections | ✅ + saved filters | ✅ | ✅ | Flutter's saved filters lost |

## 13. Flutter-only subsystems (no successor on any platform)

These shipped in Flutter and exist nowhere today — the true migration debt beyond the per-row gaps:

| Subsystem | What it was | Size |
|---|---|---|
| **Activities & conditions analysis** | Per-activity condition scoring: score hero, drivers, forecast bands, suggested time windows, warnings, provenance; 7 pluggable activity kinds (hiking, xc-ski, backcountry-ski, fishing, freediving, packrafting, + route drawing) | Largest lost subsystem |
| **"Today" recommendations** | Ranked what-to-do-today screen fed by the analysis above | Depends on the above |
| **Observations** | Per-activity field observation forms with drafts | Part of activities |
| **Curated MVT trails + external vector layers** | Curated trail tiles (hiking/ski/forest-roads/cycling) + N50 sti, Nasjonal Turbase, OSM paths as tappable vector features with offline vector-tile caching | Android has only a raster Trails overlay |
| **Custom tile providers** | User-supplied map URLs | Small |
| **MET alerts + Varsom avalanche forecast** | Live warning banners/sheets | Small–medium |
| **Photo-library map** | Whole device photo library as a map layer | Medium; Android has a different (capture-first) model |
| **Elevation backfill** | Fill missing track elevations from Kartverket høydedata | Small |

## Summary

**Coverage vs the Flutter baseline** (rows where Flutter was ✅/🟡):

- **Android** covers the overwhelming majority of Flutter's core outdoor loop — recording, planning, following, tracks, markers, photos, offline, search, weather, auth/sync — and **exceeds** it in nearly every one it covers (pause-buffer, drafts, kcal, checkpoints, silent reroute, dim guide, radar, route-along weather, Wi-Fi policy, camera persistence). Its real gaps vs Flutter: the **activities/today/observations subsystem**, **vector trail data + external layers** (raster-only overlay today), **MET alerts + Varsom forecast**, **address/kommune search**, **track line styling**, **marker export**, **custom tile URLs**, **location-marker customization**, and friends/groups (scaffolded, backend "unsupported").
- **Web** is a deliberate subset: viewer/planner/organizer, not a field tool. Recording, following, offline, and photos are out of scope by design; within scope it matches or beats Flutter (3D terrain, sun/shadows, desktop layout, keyboard shortcuts, full-pose camera restore). Its in-scope gaps: **no data overlays at all**, no route undo / least-detour insert / Line-Draw modes, no in-planner elevation profile, no marker photos, no radar, panel-only weather, stub map readout, and no WebGL2 fallback (WebGPU-only browsers).
- **New shared ground the successors added that Flutter never had:** real 3D terrain with sun, cast shadows and atmosphere, one shared Rust engine across platforms, and (Android) procedural radar clouds.

**Highest-impact follow-ups if parity with the removed app is the goal:**
1. Vector trail layers (curated MVT + Turbase) on the wgpu engine — the most user-visible daily-use loss.
2. MET alerts + Varsom avalanche forecast surfaces (safety features).
3. The activities/conditions-analysis subsystem (product differentiator; large).
4. Friends/groups backend wiring (client scaffolding already exists on Android).
5. Web: data overlays + route-planner ergonomics (undo, elevation profile); fix the `MapReadout` stub.
