# Flutter E2E tests (Patrol)

Patrol-driven user-journey tests. Each test drives the **real widgets
the user touches** — text fields, segmented buttons, save buttons,
modal sheets, confirmation dialogs, error banners — on an iOS
Simulator, against the live docker-compose backend.

These are **outside** the unit/widget test tree under `test/`. The
standard `flutter test` doesn't pick them up; they run via `patrol`
against a booted simulator with a healthy backend.

## What's tested

Three user-visible outcomes, in flows as long as the user actually
takes:

| Journey | Real UI driven |
| --- | --- |
| User signs up, saves two spots of different kinds, opens each in its detail sheet, deletes one, restarts the app, and the remaining spot is still there | RegisterScreen → long-press FlutterMap → PinOptionsSheet ("Add activity here") → ActivityCreatePicker → Fishing → FishingCreateScreen (name + Lake + Shore + Save) → tap pin → FishingDetailSheet (asserts saved name + Lake + Shore are shown back) → Close → long-press map → picker → Freediving → FreedivingCreateScreen (name + Max depth + Save) → tap pin → FreedivingDetailSheet (saved name visible) → Delete → "Delete spot?" confirmation dialog → confirm → pin gone → restart the app → only the remaining fishing pin is there. |
| A fresh sign-up does not show me the previous owner's pin | Another user has saved a spot server-side; a different user signs up through the real UI; asserts that other user's pin key never appears on the new user's map. |
| Wrong password tells me my password was wrong | LoginScreen with bad creds; asserts AuthErrorMessage banner appears and LoginScreen stays open. |

## Bug this suite caught (now fixed)

The privacy test originally failed because
`ActivitySummariesRepository` kept the previous user's pins in memory
on logout — and `_bootstrap` re-hydrated them from the local sqlite
cache on the next session, briefly showing them to a different user.
Fix lives alongside the test:
`ActivitySummariesRepository.build()` now listens to `authStateProvider`
and on a transition out of `authenticated` it wipes both the in-memory
state and the local `activity_summaries`, `conditions_cache`, and
`details_cache` tables. On a transition INTO authenticated it
re-bootstraps for the new user.

## Deliberate non-tests (and why)

- **Editing a saved spot.** No edit UI exists. Detail sheets only
  surface Close + Delete. Adding a fake "edit via API" test would
  verify code paths the user can't reach.
- **Logout via the drawer.** The drawer's Logout ListTile pops the
  drawer with `Navigator.pop(context)` and then runs an async
  confirmation dialog whose callback still uses the drawer's `ref`.
  By the time the dialog confirms, the drawer is unmounted and
  `ConsumerStatefulElement.read` throws "Using 'ref' when a widget
  is about to or has been unmounted is unsafe". This is a real app
  bug, not a test fragility — file separately. Until it's fixed, the
  privacy test uses the same logout code path the dialog's confirm
  tries to hit, via `authStateProvider.notifier.logout()`.
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
