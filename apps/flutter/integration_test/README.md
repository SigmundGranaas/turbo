# Flutter E2E tests (Patrol)

Patrol-driven user-journey tests. Each test drives the **real widgets
the user touches** — text fields, segmented buttons, save buttons,
modal sheets, confirmation dialogs, error banners — on an iOS
Simulator, against the live docker-compose backend.

These are **outside** the unit/widget test tree under `test/`. The
standard `flutter test` doesn't pick them up; they run via `patrol`
against a booted simulator with a healthy backend.

## What's tested

Five user-visible outcomes, in flows as long as the user actually
takes:

| Journey | Real UI driven |
| --- | --- |
| User signs up, saves two spots of different kinds, opens each in its detail sheet, deletes one, restarts the app, and the remaining spot is still there | RegisterScreen → long-press FlutterMap → PinOptionsSheet ("Add activity here") → ActivityCreatePicker → Fishing → FishingCreateScreen (name + Lake + Shore + Save) → tap pin → FishingDetailSheet (asserts saved name + Lake + Shore are shown back) → Close → long-press map → picker → Freediving → FreedivingCreateScreen (name + Max depth + Save) → tap pin → FreedivingDetailSheet (saved name visible) → Delete → "Delete spot?" confirmation dialog → confirm → pin gone → restart the app → only the remaining fishing pin is there. |
| User promotes a saved path to a hiking activity and sees it on their map | A saved path is persisted via the same repository the recording flow uses; the user opens it in PathDetailSheet, taps "Save as activity", picks Hiking, fills the form, taps Save. Asserts the new hiking pin's key is in the tree and tapping it surfaces the name in HikingDetailSheet. |
| User signs out from the drawer and the sign-in screen comes back | Tap menu → drawer's Logout → confirmation dialog → confirm → wait for state. Re-open menu → "Login" entry is visible. |
| A fresh sign-up does not show me the previous owner's pin | Another user has saved a spot server-side; a different user signs up through the real UI; asserts the other user's pin key never appears on the new user's map. |
| Wrong password tells me my password was wrong | LoginScreen with bad creds; asserts AuthErrorMessage banner appears and LoginScreen stays open. |

## Bugs this suite caught (now fixed alongside)

- **Cross-user privacy.** `ActivitySummariesRepository` kept the
  previous user's pins in memory on logout — and `_bootstrap`
  re-hydrated them from the local sqlite cache on the next session,
  briefly showing them to a different user. Fix:
  `ActivitySummariesRepository.build()` listens to `authStateProvider`
  and on a transition out of `authenticated` it wipes the in-memory
  state plus the local `activity_summaries`, `conditions_cache`, and
  `details_cache` tables. On a transition INTO authenticated it
  re-bootstraps for the new user.
- **Drawer logout `ref` use after unmount.** The drawer's Logout
  ListTile called `Navigator.pop(context)` then awaited the
  confirmation dialog, and the dialog's callback used the drawer's
  (now-unmounted) `ref`. `ConsumerStatefulElement.read` threw
  "Using 'ref' when a widget is about to or has been unmounted is
  unsafe". Fix: capture the notifier before the `Navigator.pop` and
  pass it into `_showLogoutDialog`.

## Deliberate non-tests (and why)

- **Editing a saved spot.** No edit UI exists. Detail sheets only
  surface Close + Delete. Adding a fake "edit via API" test would
  verify code paths the user can't reach — there's no user goal
  to test until the app grows an edit screen.
- **Backcountry-ski / xc-ski / packrafting kinds.** They share the
  exact same entry-from-saved-path → picker → form → save shape
  as hiking, with kind-specific form fields. The hiking test
  exercises that shape end-to-end. Repeating per kind multiplies
  CI minutes without adding signal — unless one of those kinds
  grows a distinct UI surface (e.g. an avalanche-warning view,
  putin/takeout map), at which point a kind-specific test earns
  its keep.
- **GPS-recorded paths (vs imported / programmatic).** The user's
  primary entry into the line-based create flow is "record a hike
  in real time, save it". iOS Simulator can simulate location, but
  driving a multi-minute walking trace from a Patrol test is
  long-running and adds no signal the imported-path flow doesn't
  already cover. We use the same `savedPathRepository.addPath()`
  call the recording flow's "Save" button uses, so the test
  exercises the same downstream code path.
- **Offline tolerance.** Patrol's `disableWifi` is Android-only —
  iOS Simulator doesn't expose a Wi-Fi toggle XCUITest can flip
  (platform limitation, not a fixable test issue). The user-visible
  behaviour ("pins served from local cache when network drops") is
  covered by `test/features/activities/
  activity_offline_behavior_test.dart` in the widget-test suite.
  Revisit if we add an Android emulator target or Patrol grows iOS
  network controls.

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

- **Edit flow.** Blocked on the app — there is no edit screen for
  saved activities. When one is added, drive the rename / details
  change → save → assert the new values are shown in the detail
  sheet.
- **Backcountry-ski / xc-ski / packrafting per-kind tests.** Only
  worth adding when one of those kinds grows a distinct UI surface
  (e.g. an avalanche layer toggle, putin/takeout map) — the
  shared shape is covered by the hiking test.
- **Route drawing via the RouteDrawingScreen surface.** The hiking
  test currently saves the activity with the path's existing
  geometry as the route (no re-draw). Add a test that opens
  RouteDrawingScreen via the form's "Draw route on map" button,
  taps a few map points to add vertices, saves, and verifies the
  resulting polyline.
- **Offline.** Re-evaluate if Patrol adds iOS Wi-Fi controls or
  we add an Android emulator target.

## CI

Not wired into `.github/workflows/flutter_tests.yaml` yet. A separate
workflow needs: macOS runner + iOS simulator boot + docker compose up
+ healthz wait + `run_e2e.sh`. Expect ~5–10 min per run after image
pulls are cached.
