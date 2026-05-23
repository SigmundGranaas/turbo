# Flutter E2E tests (Patrol)

Patrol-driven user-journey tests. Each test drives the **real widgets
the user touches** — text fields, segmented buttons, save buttons,
modal sheets, confirmation dialogs, error banners — on an iOS
Simulator, against the live docker-compose backend.

These are **outside** the unit/widget test tree under `test/`. The
standard `flutter test` doesn't pick them up; they run via `patrol`
against a booted simulator with a healthy backend.

## What's tested

Five user-visible outcomes:

| Journey | What's driven through real UI |
| --- | --- |
| New user signs up and sees their first saved spot | RegisterScreen (email + password + Create-account button), FishingCreateScreen (name field, Lake/Shore segmented buttons, Save), FishingDetailSheet (asserts the saved name + chosen options are shown back to the user) |
| Returning user can re-open their saved spot | Cold start with persisted tokens, FishingDetailSheet shows the prior spot's name |
| User deletes a spot from its detail sheet, it stays gone | Detail sheet's Delete button → Delete-spot? dialog → confirm; verifies the spot is gone server-side |
| Another user cannot see my private spot | User B opens the detail sheet for User A's spot id; user A's name never appears anywhere on B's screen |
| Wrong password tells me my password was wrong | LoginScreen with bad creds; asserts AuthErrorMessage banner appears and screen stays open |

Out of scope (yet) and why:
- **Map-tap entries** (long-press map → activity picker → kind form;
  tap a pin → detail sheet). MapLibre rendering on iOS Sim doesn't
  emit a stable Marker locator. We push the form / sheet directly via
  the Navigator — the user-facing widget behaviour is the same. When
  we have a reliable pin-locator, those entry-point gestures can be
  added on top of the existing assertions.

## Prerequisites

1. **Patrol CLI** on PATH:
   ```sh
   dart pub global activate patrol_cli
   export PATH="$PATH:$HOME/.pub-cache/bin"
   ```
2. **Xcode + an iOS simulator booted**. The runner defaults to the
   iPhone 17 sim UDID `D3A062B4-2AB3-47C0-AE42-1CFA0FECE11A`; override
   with `E2E_DEVICE=<udid>`. List sims with `xcrun simctl list devices`.
3. **CocoaPods**. iOS pod install must have run with the Patrol +
   RunnerUITests setup (handled by the Podfile in this repo).
4. **Backend stack** up:
   ```sh
   docker compose -f infra/compose/compose.yaml \
                  -f infra/compose/compose.services.yaml up -d
   # curl -fsS http://localhost:8080/healthz   # → 200
   ```

## Running

```sh
integration_test/run_e2e.sh                              # full suite
integration_test/run_e2e.sh integration_test/some_test.dart   # one file
```

Or directly:

```sh
patrol test --target integration_test/user_journeys_test.dart \
  -d D3A062B4-2AB3-47C0-AE42-1CFA0FECE11A \
  --dart-define=API_BASE_URL=http://localhost:8080
```

Each test takes 5–55s (build + sim boot are amortised across the
suite). Full suite is currently ~1 min.

## How the suite is wired together

- **Native Patrol bridge** (`ios/RunnerUITests/`): a UI test target
  added to `Runner.xcodeproj` with `PATROL_INTEGRATION_TEST_IOS_RUNNER`
  as its entry point. The Xcode scheme's `<Testables>` block includes
  this target so `xcodebuild test` finds it.
- **Pubspec config** (`pubspec.yaml` → `patrol:`): sets the test
  directory to `integration_test/` (patrol's default is `patrol_test/`).
- **`helpers/e2e_harness.dart`**: shared utilities used by the test
  file — `waitForBackendHealthy`, `registerNewUser` (direct HTTP to
  /api/auth/Auth/register), `seedAuthState`, `resetAppState`,
  `waitForServerSidePlace`, `directApi`, plus the existing
  pump/error-suppression helpers.
- **`run_e2e.sh`**: orchestrates healthz-wait → simulator boot → patrol
  test.

## Conventions

- **One test per user-visible outcome.** No state-machine assertions,
  no per-field payload checks, no HTTP status assertions. If something
  the user cares about breaks, one of these tests fails.
- **Drive what's natural, set up what's not.** Real UI for the
  thing-under-test (forms, sheets, dialogs); direct HTTP for prior
  state ("user B already had a session before User A logged in" —
  setting that up via UI would be three nested log-ins for one test).
- **Assert on what the user sees on screen.** The detail sheet's
  visible name. The error banner widget. The text the dialog asks
  before destructive actions. If the data is only verifiable via API,
  that's a hint we should be looking at a different screen.

## Gaps worth adding next

- Edit a saved spot (rename, change water kind) → the new name shows
  in the detail sheet.
- Tap a pin on the map. Requires a reliable Marker locator on the
  MapLibre layer; consider adding `Key` to marker widgets so Patrol
  can find them.
- One test per activity kind (backcountry ski, hiking, xc ski,
  packrafting, freediving), each verifying the kind-specific create
  form and detail surface.
- Localized strings — drive the LoginScreen / RegisterScreen in
  Norwegian and assert on the localized texts.
- Connectivity loss / reconnect — cut the simulator's network mid-flow
  via `$.native.disableWifi()`, save, reconnect, verify the spot
  surfaces.

## CI

Not wired into `.github/workflows/flutter_tests.yaml` yet. A separate
workflow needs: macOS runner + iOS simulator boot + docker compose up
+ healthz wait + `run_e2e.sh`. Expect ~5–10 min per run after image
pulls are cached.
