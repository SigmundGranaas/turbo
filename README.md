# Turkart

An offline-capable hiking and outdoors map application built with Flutter.
Targets iOS, Android, web, and desktop from a single codebase.

## What's here

- **Live GPS recording** — start a track on the map, walk, stop. Background-capable on iOS/Android with the screen off.
- **Elevation profiles** — every recorded path captures altitude per fix; sparkline + ascent/descent stats on the path detail.
- **Import / export** — load tracks from GPX, GeoJSON, or KML; export the same. Multi-track GPX files split into multiple paths.
- **Markers with photos** — drop pins, attach camera or gallery photos. Photos live on-device under the app's documents directory.
- **Collections** — organize markers and paths into named groups, including smart collections defined by a saved filter.
- **Offline maps** — download tile regions for use without a network connection. Per-region size and tile count estimates before download.
- **Search** — local marker search plus Kartverket placename lookups, including reverse-geocoding on long-press.
- **Navigation** — point-to-point compass guidance to any marker or long-pressed point on the map.

## Architecture

Feature-Oriented Architecture. Each top-level folder under `lib/features/` is a feature with a single public façade at `<feature>/api.dart`. Cross-feature imports go through `api.dart` only; this is enforced by `test/architecture/feature_boundary_test.dart`.

State is hand-written Riverpod (no codegen). Persistence is dual-backend: SQLite on mobile/desktop via `sqflite`, IndexedDB on web via `idb_shim`. The schema version lives in `lib/core/data/database_provider.dart` with `_migrateVNToVN1` helpers for each bump.

Full architecture notes: [`lib/context/architecture.context.md`](lib/context/architecture.context.md).

## Running it

```sh
flutter pub get
flutter run                 # picks the connected device / running emulator
flutter test                # full unit + widget suite
flutter analyze             # static analysis
```

For background recording on a physical Android device, location permission must be granted to **Always** (foreground-only still records while the app is in front, just stops if the screen locks). On iOS, accept the "Always Allow" prompt on the second permission ask.

## Project layout

```
lib/
  app/          — top-level shell, theming, localization
  core/         — cross-feature primitives: db, location, util, sharing
  features/    — one folder per feature, each with api.dart, data/, models/, widgets/
test/
  features/    — API behavioral + end-to-end widget tests, one folder per feature
  architecture/ — boundary tests that enforce feature isolation
```

## Contributing

Every new feature needs an `api.dart` façade; the boundary test will fail otherwise. Every new notifier needs an API-level behavioral test; every new user flow needs an end-to-end widget test.

Run `flutter analyze` and `flutter test` before pushing.
