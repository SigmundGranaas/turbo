# Turbo · iOS

A native iPhone reimagining of Turbo (the Norwegian hiking-map app) in Apple's
**iOS 26 "Liquid Glass"** language. SwiftUI + Swift Package Manager.

This is the baseline + base architecture. It **mirrors the native Android app**
(`apps/android`) — same layered, multi-module shape, same MVVM + DI conventions,
same domain contracts — so the two clients stay in step. The design is the
`Turbo · iOS 26` handoff from Claude Design (SF Pro, system colors, Liquid Glass,
MapKit-style chrome).

## Architecture

A thin Xcode app target assembles a modular Swift package. Each Swift package
target maps one-to-one onto an Android Gradle module:

| Swift target        | Android module        | Responsibility |
|---------------------|-----------------------|----------------|
| `CoreModel`         | `:core:model`         | Pure domain + geo types (`LatLng`, `ActivityKindId`, `Marker`, `BaseLayer`, `UserSettings`, `SavedPath`, `MapCollection`, `SearchHit`, `GeoPath`/`GeoMetrics`). No UI. |
| `CoreCommon`        | `:core:common`        | `Outcome` result type + `ReactiveStore` (the `StateFlow` analogue). |
| `CoreDesignSystem`  | `:core:designsystem`  | iOS 26 theme tokens (`TurboColors`, typography, spacing), the `liquidGlass` material, shared components (`Glyph`, `Monogram`, `Spark`, the map control chrome), activity-kind visuals. |
| `CoreData`          | `:core:data`          | Repository seams + real backends — SwiftData (`Marker`/`Path`/`Collection`), `UserDefaults` settings, Kartverket stedsnavn search — each with an in-memory impl behind the same protocol. |
| `CoreAuth`          | `:core:auth`          | `AuthState` + `AuthRepository`; `GoogleAuthRepository` (ASWebAuthenticationSession + API token exchange) and an in-memory default. |
| `CoreSync`          | `:core:sync`          | `SyncDecisions` (last-write-wins), `MarkerSyncEngine`, `MarkerSyncTransport` (HTTP + in-memory), `SyncController` (foreground-gated). |
| `CoreMap`           | `:core:map`           | The map SDK boundary: `TurboMapView` (MKMapView + `MKTileOverlay` raster tiles), tile styles, and the `OfflineTileManager` seam (`InMemoryOfflineTileManager`). |
| `FeatureMap`        | `:feature:map`        | The map home (`MapScreen`) + `MapViewModel` — full-bleed map with the Liquid Glass control rail, search, FAB, markers. |
| `FeatureLayers`     | `:feature:layers`     | `MapLayersSheet` — base-map picker + overlay toggles. |
| `FeatureSearch`     | `:feature:search`     | `SearchScreen` + `SearchViewModel` — place search + recents. |
| `FeatureSettings`   | `:feature:settings`   | `SettingsScreen` + `SettingsViewModel` (theme/units/sharing). |
| `FeatureRecording`  | `:feature:recording`  | `PathsScreen` + `PathsViewModel` — recorded tracks with sparklines. |
| `FeatureAuth`       | `:feature:auth`       | `AuthScreen` + `AuthViewModel` — Sign in with Apple. |
| `FeatureCollections`| `:feature:collections`| `CollectionsScreen` + `CollectionsViewModel`. |
| `FeatureOffline`    | `:feature:offline`    | Offline-maps screen (`OfflineMapsScreen`) + `OfflineViewModel`. |
| `TurboApp`          | `:app`                | Composition root (`AppContainer`) + root navigation (`RootView`). |

### Conventions (mirrored from Android)

- **MVVM.** A feature is a SwiftUI `View` + an `@Observable @MainActor` view
  model. `@Observable` is the Swift equivalent of a Hilt `ViewModel` exposing a
  `StateFlow`; an `AsyncStream` off a seam mirrors a `Flow`.
- **DI.** Constructor injection. `AppContainer` is the hand-rolled equivalent of
  the Hilt graph — it owns singletons and vends `make<Feature>ViewModel()`
  factories. Feature modules contain no construction logic.
- **Seams.** SDK-specific work (offline tiles, later the live map) sits behind a
  protocol in `CoreMap`, with a swappable implementation. Today's
  `InMemoryOfflineTileManager` lets the feature run end-to-end (and unit-test)
  before MapKit/MapLibre is wired in.
- **Pure core.** `CoreModel` keeps the same domain contracts and Norwegian
  activity keys as Android so both clients and the API agree.

## Build & run

Requires Xcode 26 and [XcodeGen](https://github.com/yonaskolb/XcodeGen)
(`brew install xcodegen`).

```sh
cd apps/ios
xcodegen generate                       # (re)generate Turbo.xcodeproj from project.yml
open Turbo.xcodeproj                     # …or build from the CLI:
xcodebuild -project Turbo.xcodeproj -scheme Turbo \
  -sdk iphonesimulator -destination 'generic/platform=iOS Simulator' build
```

The `.xcodeproj` is generated from `project.yml` and git-ignored — edit
`project.yml`, not the project.

### Tests

**One command runs the whole stack** — generate project, unit suite, boot a
simulator, E2E suite — and exits non-zero on any failure:

```sh
apps/ios/scripts/test.sh            # everything
apps/ios/scripts/test.sh --unit-only
apps/ios/scripts/test.sh --e2e-only
```

It picks an available iPhone simulator automatically and retries the (occasionally
flaky) swift-testing spin-up. The E2E suite launches the app with `-uitest`, which
swaps `AppContainer` to deterministic, seeded **in-memory** backends — so the UI
tests are hermetic (no live network, Kartverket, or OAuth).

Two layers:

**Unit / integration** (fast, host, no simulator) — domain logic, repositories,
the `repository → view-model → state` spine, and serialisers (track export).
Runs with SwiftPM:

```sh
cd apps/ios
swift test            # 92 tests (Swift Testing)
```

**End-to-end UI** (XCUITest, `TurboUserFlowsUITests`) — launches the real app and
drives Turbo's **core user flows**, each test named for a hiker's goal and
asserting only on the *outcome*, not widget mechanics:

- opens the app and lands on the map
- finds a place by searching and chooses it
- searching for a place **recenters the map** on it (camera move asserted)
- saves a spot and it appears on the map
- **browses their saved markers** (My Markers)
- reviews their recorded hikes
- exports a recorded track (share sheet opens)
- switches the base map and the choice sticks
- downloads a region for offline use
- preferences are remembered across navigation
- signs in to their account
- records a track and saves it to Paths
- opens a hike detail / a marker detail
- checks the weather and avalanche danger
- **full journey** — search → save that place → see it in My Markers → export it

CI runs the SwiftPM suite + the E2E suite on every PR
(`.github/workflows/ios_build.yml`).

Runs via xcodebuild:

```sh
cd apps/ios
xcodegen generate
xcodebuild test -project Turbo.xcodeproj -scheme Turbo \
  -destination 'platform=iOS Simulator,name=iPhone 16'
```

New features are developed **test-first** (red → green → refactor); see
`TrackExportTests` / `MarkerEditorViewModelTests` for the pattern.

## Roadmap

All core modules, the primary feature set, **and real backends behind every
seam** are in place: SwiftData + UserDefaults persistence, Kartverket search, an
on-disk offline tile downloader, `CoreSync` (last-write-wins, foreground-gated),
and Google sign-in (`ASWebAuthenticationSession`). The in-memory impls remain as
the default for previews/tests; production wiring lives in `AppContainer`.

What remains, iteratively:

- **Go live (config-only).** The plumbing is done — `AppContainer` uses
  `GoogleAuthRepository` + token-authed `HttpSyncTransport` when `TurboAPIBaseURL`
  is set, and the `turbo` OAuth scheme is in the Info.plist. Set
  `TurboAPIBaseURL` + supply a Google client id to flip it on.
- **Real provider backends** behind the existing seams: WeatherKit
  (`WeatherProvider`), Varsom/NVE (`AvalancheProvider`), MapKit/MapLibre tiles.
- **Recording Live Activity / Dynamic Island** (needs a Widget Extension target).
- **Localization (nb-NO).** Remaining dedicated pass: add `defaultLocalization`
  to `Package.swift`, a `Localizable.xcstrings` per UI module under a `Resources`
  folder (`.process` in the target), and route each `Text` through
  `bundle: .module`. Deferred deliberately — it touches every module and a
  partial pass would ship a mixed-language UI.

Done: detail surfaces (hike/marker/weather/avalanche); map interactions (pin tap,
long-press menu, overlays, compass reset); live location + heading; the offline
loop; track recording; sync (markers + paths + collections) with a persisted
cursor; the go-live config gate + token wiring; and CI.
