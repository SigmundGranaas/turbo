# Flutter E2E tests (Patrol)

Patrol-driven user-journey tests. Each test drives the **real widgets
the user touches** — text fields, segmented buttons, save buttons,
modal sheets, confirmation dialogs, error banners — on an iOS
Simulator, against the live docker-compose backend.

These are **outside** the unit/widget test tree under `test/`. The
standard `flutter test` doesn't pick them up; they run via `patrol`
against a booted simulator with a healthy backend.

## What's tested

Four user-visible outcomes, in flows as long as the user actually
takes:

| Journey | Real UI driven |
| --- | --- |
| New user signs up, saves a fishing spot AND a freediving spot via the map, sees both pins | RegisterScreen (email + password + Create-account button) → long-press FlutterMap → PinOptionsSheet ("Add activity here") → ActivityCreatePicker (Fishing/Freediving tiles) → FishingCreateScreen (name + Lake + Shore + Save) → second long-press → picker → Freediving → FreedivingCreateScreen (name + Max depth + Save). Asserts: both `activity-pin-<id>` keys are present on the map. |
| Returning to the app, both previously-saved pins are on the map | Cold start with a seeded session that has two prior spots. Asserts: both pin keys appear once the summaries refresh. |
| Another user's spots are not on my map | User A has a spot; User B signs in fresh, opens the app. Asserts: User A's pin key is never on User B's map, even after the repo has had time to refresh. |
| Wrong password tells me my password was wrong | LoginScreen with bad creds. Asserts: AuthErrorMessage banner appears, LoginScreen stays open. |

## Deliberate non-tests (and why)

- **Editing a saved spot.** No edit UI exists. Detail sheets only
  surface Close + Delete. Adding a fake "edit via API" test would
  verify code paths the user can't reach.
- **Tapping a pin to open its detail sheet.**
  `ActivityKindDescriptor.buildDetailScreen` is defined on every kind
  but the marker rendered by `ActivitiesMapLayer` has no `onTap` —
  the pin is non-interactive in the production UI. The detail sheets
  exist as widgets but the user can't open them. Test will be added
  once the marker tap is wired.
- **Deleting a saved spot.** Delete only exists on the detail sheet,
  and the detail sheet isn't reachable (see above).
- **Line-based kinds** (hiking, backcountry-ski, xc-ski, packrafting).
  They need a route-drawing UI; a single long-press cannot create
  them. The two point-based kinds (fishing, freediving) cover all the
  UI surfaces that have no preconditions outside a single point.
- **Offline tolerance.** Patrol's `disableWifi` is Android-only —
  iOS Simulator doesn't expose a Wi-Fi toggle XCUITest can flip. The
  user-visible behaviour ("pins served from local cache when network
  drops") is covered by `test/features/activities/
  activity_offline_behavior_test.dart` in the widget-test suite. Add
  the E2E variant when Patrol supports iOS Wi-Fi toggling or we move
  E2E to an Android emulator.

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

These are blocked on UI surfaces the user can't currently reach (the
test wouldn't represent a real user goal):

- **Detail-sheet rendering after pin tap.** Add `onTap` to the
  `ActivitiesMapLayer` marker — opens
  `descriptor.buildDetailScreen(ctx, id)` in a `showModalBottomSheet`
  — then test: tap pin → sheet → asserts on the saved name + the
  options the user chose in the form.
- **Delete a saved spot.** Same precondition as above. The delete
  button + confirm dialog already exist on the detail sheets; once
  the sheets are reachable they can be tapped.
- **Edit a saved spot.** No edit screen exists in the app yet. When
  one is added, drive the rename → save → assert the new name is
  shown back to the user.
- **Line-based kinds** (hiking, backcountry-ski, xc-ski, packrafting).
  Drive the route-drawing UI from a long-press / draw-mode toggle,
  save, then verify the polyline + kind-specific detail surface.

## CI

Not wired into `.github/workflows/flutter_tests.yaml` yet. A separate
workflow needs: macOS runner + iOS simulator boot + docker compose up
+ healthz wait + `run_e2e.sh`. Expect ~5–10 min per run after image
pulls are cached.
