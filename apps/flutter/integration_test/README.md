# Flutter integration (E2E) tests

End-to-end user stories that drive the real `TurboApp` against the real
backend stack (the docker compose setup in `infra/compose/`). Crosses every
layer the live app does: Dio + JWT interceptor + gateway + per-module .NET
host + Postgres + (modulith bus or NATS subscriber) + projection.

These tests are deliberately **outside** the unit/widget test tree under
`test/`. The standard `flutter test` command does **not** pick them up;
they need an explicit device (`-d macos` works headless on Apple Silicon
without an emulator) and a running backend.

## Prerequisites

1. Docker Desktop running.
2. Backend stack up (one-time per session):
   ```sh
   docker compose -f infra/compose/compose.yaml \
                  -f infra/compose/compose.services.yaml up -d
   # Wait until: curl -fsS http://localhost:8080/healthz   (returns 200)
   ```
3. macOS deps in place: Flutter ≥ 3.41, Xcode + CocoaPods (the harness
   uses `flutter pub get`, no Pod install needed for `-d macos`).

## Running

From `apps/flutter/`:

```sh
# One file at a time:
flutter test integration_test/login_flow_e2e_test.dart \
  -d macos --dart-define=API_BASE_URL=http://localhost:8080
```

`integration_test` on desktop only supports one app instance per
`flutter test` invocation — running the whole `integration_test`
directory fails the second file with "Unable to start the app on the
device". The `run_e2e.sh` helper in this directory waits for healthz
then invokes each `*_test.dart` file separately:

```sh
integration_test/run_e2e.sh                                # all files
integration_test/run_e2e.sh integration_test/login_flow_e2e_test.dart
```

## Conventions

- **Unique users per test**: helpers in `helpers/e2e_harness.dart` mint
  timestamp-tagged emails (`e2e-<tag>-<microseconds>@turbo.test`). The
  suite is idempotent — re-running it never collides — and the compose
  stack doesn't need a DB reset between runs.
- **Auth seeding**: tests that don't exercise the login UI use
  `registerNewUser()` + `seedAuthState()` to start in an authenticated
  cold-state. Tests that *do* exercise login use the form drivers
  (`submitLoginForm`).
- **No mocks**: this layer is for the things widget tests can't catch —
  wire-format mismatches, JWT signing, real persistence, cross-service
  authorization. Use widget tests under `test/` for everything else.
- **Map-driven flows are stubbed-out**: `MainMapPage` renders but tests
  do not depend on MapLibre tiles loading. UI flows that would start
  from a long-press on the map call the repository or push the screen
  directly. Cover the *interaction* logic in widget tests; cover the
  *end-to-end contract* here.

## What's covered

| File | User stories |
| --- | --- |
| `login_flow_e2e_test.dart` | Register from the LoginScreen → authenticated. Login with valid creds. Login with wrong password → error + screen stays. Logout from authenticated state. |
| `activity_lifecycle_e2e_test.dart` | Create fishing activity → optimistic upsert + server projection. Delete → tombstone + 404 on detail. Cross-user isolation: user B cannot read user A's activity. |

## Gaps worth adding next

- Token refresh on 401: drive a request after deliberately expiring the
  access token (server-side test hook needed, or wait 15+ min).
- Offline → online resync: drop the device's network, create activity
  (should fail), reconnect, refresh, see clean state.
- Cross-kind: create one activity of each kind, assert all surface in
  the summary store, assert kind filtering works.
- Update flow: edit name/details, refresh, assert version bump and new
  values propagate.

## CI

Not wired into the existing `.github/workflows/flutter_tests.yaml` yet
(which runs only `flutter test` on Ubuntu). A separate workflow needs:
docker compose up + healthz wait + `flutter test integration_test`. The
runtime is dominated by image pulls; expect ~3–5 min per run after the
first.
