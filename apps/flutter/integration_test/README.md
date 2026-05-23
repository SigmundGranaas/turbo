# Flutter integration (E2E) tests

End-to-end user-journey tests that drive the real `TurboApp` against the
real backend stack (the docker compose setup in `infra/compose/`).

**What's in scope**: assertions about value the user can see or actions
they can take. "I signed up and my place shows up." "I deleted it and
it stayed gone." "Wrong password tells me so." Nothing else.

**What's out of scope**: assertions about internal state machines,
endpoint status codes, optimistic-upsert timing, etc. Those layers are
implicit — if any of them break, a user-journey test breaks. Layer-
specific assertions belong in unit/widget tests under `test/` and in the
.NET behaviour tests under `apps/api/tests/`.

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
flutter test integration_test/user_journeys_e2e_test.dart \
  -d macos --dart-define=API_BASE_URL=http://localhost:8080
```

`integration_test` on desktop only supports one app instance per
`flutter test` invocation, so running the whole `integration_test`
directory fails the second file with "Unable to start the app on the
device". The `run_e2e.sh` helper in this directory waits for healthz
then invokes each `*_test.dart` file separately:

```sh
integration_test/run_e2e.sh                                       # all
integration_test/run_e2e.sh integration_test/user_journeys_e2e_test.dart
```

## Conventions

- **One test per user-visible outcome.** No infra assertions, no
  intermediate state-machine checks, no per-field payload verification.
  If any of those break, one of the user-journey tests fails — that's
  the signal.
- **Unique users per test.** Helpers in `helpers/e2e_harness.dart` mint
  timestamp-tagged emails (`e2e-<tag>-<microseconds>@turbo.test`). The
  suite is idempotent — re-running it never collides — and the compose
  stack doesn't need a DB reset between runs.
- **Drive what's natural, set up what's not.** Use the real UI for the
  gesture under test (register, login, error feedback). For the prior
  state a journey starts from — "I've already saved a place before" —
  set up via direct HTTP (`directApi()`) instead of clicking through
  the create form, since that's setup, not the thing being tested.
- **Assert against the summary store, not the map widget.** The map
  reads from `activitySummariesRepositoryProvider`; the store is the
  source of pin data the user sees. MapLibre tiles don't render
  reliably in headless macOS, so the harness suppresses tile errors
  and tests assert on the source-of-truth provider.

## What's covered

| Journey | File |
| --- | --- |
| A new user signs up and their first saved place is visible to them | `user_journeys_e2e_test.dart` |
| Returning to the app, my previously saved places are still visible | `user_journeys_e2e_test.dart` |
| I can delete a place I saved and it stays gone | `user_journeys_e2e_test.dart` |
| A wrong password tells me my password was wrong | `user_journeys_e2e_test.dart` |
| My saved places are not visible to other users | `user_journeys_e2e_test.dart` |

## Gaps worth adding next

Each should be a single user-journey test, not a layered set:

- "After enough idle time the app still works without making me sign in
  again." (Token refresh, expressed as a user outcome.)
- "If I lose connectivity while saving, then come back online, my place
  is there next time I open the app." (Offline-resilience as outcome.)
- "If I rename a saved place, the new name is what I see." (Update
  flow.)
- "Each activity kind I save shows up as the right kind on my map."
  (Cross-kind coverage.)

## CI

Not wired into the existing `.github/workflows/flutter_tests.yaml` yet
(which runs only `flutter test` on Ubuntu). A separate workflow needs:
docker compose up + healthz wait + `run_e2e.sh`. The runtime is
dominated by image pulls; expect ~3–5 min per run after the first.
