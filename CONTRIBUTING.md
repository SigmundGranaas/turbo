# Contributing to Turkart

Thanks for the interest. The README is the right place to start — it
covers what the app does, how to run it, and the high-level layout.
This file is for things you need to know once you're actually changing
code.

## Development setup

```sh
flutter pub get
flutter run                 # picks the connected device / running emulator
flutter test                # full unit + widget suite
flutter analyze             # static analysis
```

Flutter SDK requirement is `>=3.11.0 <4.0.0` (see `pubspec.yaml`).

For background location recording on a physical Android device, location
permission must be granted to **Always**. On iOS, accept the
"Always Allow" prompt on the second permission ask.

## Architecture rules

Turkart uses Feature-Oriented Architecture. Each top-level folder under
`lib/features/` is a feature with a single public façade at
`<feature>/api.dart`. Cross-feature imports go through `api.dart` only.

These rules are enforced by tests under `test/architecture/`:

- `feature_boundary_test.dart` — no file in `lib/features/<A>` imports
  another feature except through that feature's `api.dart`. Every
  `api.dart` is a pure re-export facade.
- `no_print_test.dart` — no unguarded `print(...)` or `debugPrint(...)`
  call sites in `lib/features/`. Use the `package:logging` `Logger`
  instead; see `lib/core/service/logger.dart`.

If you're adding a new feature: create `lib/features/<name>/api.dart`
that re-exports your feature's public surface, and add an
`api_behaviour_test.dart` under `test/features/<name>/` that exercises
the feature only through that façade.

## Logging

Don't reach for `print` or `debugPrint`. The app has a shared logger:

```dart
import 'package:logging/logging.dart';
final _log = Logger('MyFeature');

_log.fine(() => 'expensive lazy message: $details');
_log.warning('something went wrong', error, stack);
```

Output is gated to `kDebugMode` by `setupLogging()` in `main.dart`. Core
code (`lib/core/`) imports the shared `log` singleton from
`lib/core/service/logger.dart`.

## Localization

User-facing strings live in `lib/app/l10n/app_localizations.dart` as a
hand-written abstract class with `AppLocalizationsEn` and
`AppLocalizationsNo` subclasses. Add the key in three places (abstract +
both subclasses) and access it via `context.l10n.<key>`.

Don't show raw exception text to users — use a friendly localized
message and log the details.

## Tests

- Every new notifier gets an API-level behavioral test.
- Every new user flow gets an end-to-end widget test.
- Database changes get a migration test (see existing
  `_migrateVNToVN1` patterns in `lib/core/data/database_provider.dart`).

Run `flutter analyze` and `flutter test` before pushing.

## Commit style

Short imperative subjects (no period). Optional `feat(scope):` /
`polish(scope):` / `fix(scope):` prefix when it adds clarity. The body
should explain *why* rather than *what* — a glance at the diff already
shows the what.

## Changelog

Add an entry under the `[Unreleased]` section of `CHANGELOG.md` for
anything user-visible. Internal refactors that don't change behavior
can be skipped.
