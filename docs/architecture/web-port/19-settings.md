# 19 — Settings

> Give a web user the same preferences the Android app has — theme, units, compass orientation, default location-follow, cloud-sync toggle, about, and language — persisted across reloads and (when signed-in) shareable across devices.

## Status
- **Android (gold standard):** `SettingsScreen` + `SettingsViewModel` over `DataStoreSettingsRepository` (DataStore `user_settings`):
  - **Theme** — `ThemeMode { System, Light, Dark }`, key `theme_mode`, default `System`; drives Material 3 `ColorScheme`.
  - **Units** — `metric_units` Boolean, default `true` (metric); consumed by all distance/elevation formatting.
  - **Compass orientation** — `compass_orientation` Boolean, default `true`; map rotates vs north-up.
  - **Default location-follow** — `follow_location` Boolean, default `false`.
  - **Cloud-sync toggle** — `cloud_sync_enabled` Boolean, default `true`; gates the sync engine (doc 18).
  - **About** — app version + links.
  - **i18n** — English + Norwegian (`values/` + `values-nb/`), follows device locale via Android string resources.
  - (Wi-Fi-only downloads `download_wifi_only` is **offline-scope → out of scope** here.)
- **Web today:** **not implemented.** No settings UI, no theme handling, no i18n library, no localStorage persistence; Zustand holds only `baseLayer` + `threeD` in memory.
- **Renderer/back-end prerequisites:**
  - **No renderer change required**, but two settings *feed* the renderer: compass orientation (bearing-lock vs free rotate) and default location-follow (initial follow camera) — both go through existing `set_camera`/follow paths (docs 02/05).
  - Persistence: web has no DataStore → use **`localStorage`** (per-device, works signed-out) and, when signed-in, mirror to **account/server settings** (the same settings field strategy referenced by docs 01/18). No new endpoint required beyond a settings read/write if server-side settings are adopted (see Open questions).
  - i18n: an i18n lib (e.g. `react-i18next`) with `no`/`en` bundles, defaulting to `navigator.language`.

## User stories

### 1. Change theme
*As a user, I want to choose System / Light / Dark, so that the app matches my preference or environment.*

**Acceptance criteria**
- A theme control offers System (default), Light, Dark.
- Selecting applies immediately: Light/Dark force the palette; System follows the OS via `prefers-color-scheme` and updates live when the OS theme changes (`matchMedia('(prefers-color-scheme: dark)')` listener).
- The choice persists across reload (localStorage; synced to account when signed-in) and is applied **before first paint** so there's no light-flash on a dark-preferring user.
- Theme is applied via CSS `color-scheme` + a `data-theme`/class on the root and a Zustand `theme` slice; all themed UI reads from CSS variables.

**Web-specific notes**
- Read the stored theme synchronously during store init and set the root attribute before React mounts (a tiny inline boot script) to avoid FOUC.

### 2. Switch units everywhere
*As a user, I want metric or imperial, so that distances and elevations read the way I expect across the whole app.*

**Acceptance criteria**
- A units toggle (Metric · km, m / Imperial · mi, ft), default metric.
- Changing it updates **every** distance/elevation/speed display app-wide immediately (track detail, elevation profile, routing summary, recording metrics, search results) — one shared formatter reads the units slice.
- Persists across reload (localStorage; synced when signed-in).

**Web-specific notes**
- Implement a single `formatDistance`/`formatElevation`/`formatSpeed` util bound to the `units` slice (one-state-one-widget) so no screen formats units independently.

### 3. Toggle compass orientation and default follow
*As a user, I want to set whether the map rotates with the compass and whether it follows my location by default, so that the map behaves the way I navigate.*

**Acceptance criteria**
- Compass-orientation toggle (default on): when on, the map can rotate to bearing; when off, north-up. Changing it updates the live camera behavior (docs 02/05) without reload.
- Default location-follow toggle (default off): when on, opening the map starts in follow mode (centered on my location, doc 05) if geolocation permission allows; when denied, it falls back to the default camera without error.
- Both persist across reload (localStorage; synced when signed-in).

**Web-specific notes**
- Compass on the web has no device magnetometer in most desktop browsers; "compass orientation" governs the map's free-rotate vs north-lock behavior and, where available, `deviceorientation`/heading (doc 05). State the degradation honestly: no hardware compass on desktop.

### 4. Enable / disable cloud sync
*As a user, I want to turn cloud sync on or off, so that I control whether my data goes to my account.*

**Acceptance criteria**
- A cloud-sync toggle (default on) with a subtitle explaining it backs up tracks, markers, and collections to your account.
- The toggle is only meaningful signed-in; signed-out it's shown disabled with a "Sign in to enable sync" hint.
- Flipping it changes the sync behavior defined in doc 18 (and the sync-status indicator reflects "Sync off").
- Persists across reload (localStorage; synced when signed-in).

**Web-specific notes**
- Exact semantics of "off" on the web (stop background refetch vs stop all account I/O) are defined in doc 18's Open questions; this doc only owns the toggle UI + persistence.

### 5. Read About
*As a user, I want an About section, so that I can see the version and find links.*

**Acceptance criteria**
- About shows the web app version (from build-time `import.meta.env` / package version) and links (e.g. Kartverket/MET attribution, privacy, source/help).
- Links open in a new tab; attribution text is present where licensing requires it.

### 6. Switch language
*As a user, I want the app in Norwegian or English, so that I can read it in my language.*

**Acceptance criteria**
- Language defaults to `navigator.language` (`no`/`nb` → Norwegian, otherwise English), matching Android "follows device".
- A language selector offers System (follow browser), English, Norwegian; changing it re-renders all translated UI immediately without reload.
- The choice persists across reload (localStorage; synced when signed-in).
- All user-facing settings strings (and shared UI) come from the i18n bundles — no hardcoded English in settings.

**Web-specific notes**
- Use `react-i18next` (or equivalent) with `en` + `no` resource bundles; detect via `navigator.language` with a stored override. Mirror Android's settings string keys where practical (e.g. theme/units/compass/follow/sync/about labels).

## Primary flows (web)

**Happy path — change a setting**
1. User opens Settings, flips Dark theme.
2. `settings` slice updates → root `data-theme="dark"` + CSS vars swap immediately.
3. Persist to localStorage; if signed-in, mirror to account settings.
4. Reload → boot script reads localStorage and applies dark before first paint.

**Edge — system theme following**
- Theme = System → app follows `prefers-color-scheme`; toggling the OS to dark updates the app live via the `matchMedia` listener.

**Edge — signed-out persistence**
- All settings persist in localStorage per-device; no account write attempted.

**Edge — signed-in cross-device**
- On sign-in, account settings are read; if they differ from localStorage, account wins (and updates localStorage) so the user sees the same prefs on every browser. *(Confirm precedence in Open questions.)*

**Edge — units propagation**
- Switching to Imperial updates an already-open track detail + elevation profile without remount (the formatter subscribes to the slice).

## UI / UX on web
- **Where:** a Settings screen reachable from the account menu / app shell (side panel on desktop, full route on mobile).
- **Layout:** grouped rows — Appearance (theme), Units, Map (compass, default follow), Sync (cloud-sync toggle), Language, About — mirroring Android's `SettingsScreen` grouping and icons.
- **Composition with canvas:** none directly except compass/follow, which affect camera behavior live; the settings panel itself overlays the shell (mobile full sheet covers the map, no inset needed).
- **Responsive:** rows stack on narrow viewports; toggles are large touch targets.

## Data & APIs
- **Persistence (signed-out / per-device):** `localStorage` keys, e.g. `turbo.theme`, `turbo.units`, `turbo.compass`, `turbo.followDefault`, `turbo.cloudSync`, `turbo.lang`. (Mirrors Android DataStore keys conceptually: `theme_mode`, `metric_units`, `compass_orientation`, `follow_location`, `cloud_sync_enabled`.)
- **Persistence (signed-in):** mirror to account/server settings (same approach docs 01/18 reference). Endpoint TBD (see Open questions) — likely a settings GET/PUT under the account; until then localStorage is canonical.
- **State:** a Zustand `settings` slice (theme/units/compass/followDefault/cloudSync/lang) with localStorage hydration on init; consumed app-wide. Cloud-sync toggle is read by doc 18; baseLayer (doc 01) lives in the same persisted settings concept.
- **i18n:** `react-i18next` with `en`/`no` bundles; detector = stored override → `navigator.language`.
- **External:** none (About links to Kartverket/MET attribution, privacy, help).

## Renderer integration
- **None new.** Compass orientation and default-follow route through existing camera/follow paths (`set_camera`, follow logic — docs 02/05). Theme/units/language/sync are app-shell only; the canvas is theme-agnostic.

## Out of scope (this phase)
- **Wi-Fi-only downloads** and any offline-download preferences (offline phase).
- Per-feature granular settings beyond the Android parity set (e.g. tile cache size, advanced renderer toggles).
- A server settings schema beyond mirroring the above fields (if server settings aren't ready, localStorage is canonical this phase).

## Open questions
- **Server settings:** is there (or should there be) an account settings GET/PUT endpoint to sync prefs across devices, or is web settings localStorage-only this phase? (Android = per-device DataStore.)
- **Precedence on sign-in:** when localStorage and account settings differ, which wins? (Proposal: account wins, updates localStorage.)
- **i18n library:** confirm `react-i18next` (vs lighter `@lingui`/custom) and whether to reuse/extract Android's string keys for a shared catalog.
- **Compass on desktop:** confirm the intended behavior when no heading source exists (north-up free-rotate only?) — tie to doc 05.
- **Version source:** where does the About version come from (package.json injected at build vs a `/version` endpoint)?
