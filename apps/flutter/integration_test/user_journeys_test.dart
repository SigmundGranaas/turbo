// Patrol-driven user-journey tests. Run on iOS Simulator against a live
// docker-compose stack. Each test drives the *real* widgets and
// gestures the user uses — long-press on the map, pin-options sheet,
// activity picker, kind-specific create form, pin tap, detail sheet,
// delete confirmation, drawer-open + logout, error banners — and
// asserts on what the user actually sees: pins on the map, the
// detail sheet's saved fields, the absence of another user's pin.
//
// Run via the runner (which waits for /healthz first):
//   integration_test/run_e2e.sh

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:latlong2/latlong.dart';
import 'package:patrol/patrol.dart';
import 'package:shared_preferences/shared_preferences.dart';
import 'package:turbo/features/activity_fishing/api.dart' as fishing;
import 'package:turbo/features/activity_freediving/api.dart' as freediving;
import 'package:turbo/features/auth/api.dart';
import 'package:turbo/features/auth/widgets/auth_error_message.dart';
import 'package:turbo/features/map_view/api.dart';
import 'package:turbo/features/saved_paths/api.dart' as paths;

import 'helpers/e2e_harness.dart' as harness;

void main() {
  patrolTest(
    'a user signs up, saves two spots, opens each, deletes one, and the '
    'remaining spot stays after a restart',
    config: const PatrolTesterConfig(visibleTimeout: Duration(seconds: 60)),
    ($) async {
      await harness.waitForBackendHealthy();
      await harness.resetAppState();
      await harness.pumpTurboApp($.tester);
      await _waitForAppReady($);

      // 1. Sign up through the real RegisterScreen.
      _pushScreen($, (_) => const RegisterScreen());
      await $(RegisterScreen).waitUntilVisible();
      await $(TextField).at(0).enterText(harness.uniqueEmail('lifecycle'));
      await $(TextField).at(1).enterText('CorrectHorse-Battery-Staple-1!');
      await $(FilledButton).tap(); // "Create account"
      await $(MainMapPage).waitUntilVisible();

      // 2. First spot — fishing — via the real map gesture.
      final fishId = await _createPointActivity(
        $,
        kindLabel: 'Fishing',
        fillForm: () async {
          await $(TextField).at(0).enterText('First Fjord');
          await $('Lake').tap();
          await $('Shore').tap();
        },
      );
      await _waitForPin($, fishId);

      // Tap the pin → kind-specific detail sheet shows the data the
      // user just entered. waitUntilVisible polls past the sheet's
      // loading state until the per-activity provider has fetched.
      await _tapPin($, fishId);
      await $(fishing.FishingDetailSheet).waitUntilVisible();
      await $('First Fjord').waitUntilVisible();
      expect($('Lake'), findsOneWidget);
      expect($('Shore'), findsOneWidget);
      await $('Close').tap();
      await $(MainMapPage).waitUntilVisible();

      // 3. Second spot — freediving — via the same real gesture path.
      final freeId = await _createPointActivity(
        $,
        kindLabel: 'Freediving',
        fillForm: () async {
          await $(TextField).at(0).enterText('Deep Cave');
          // Freediving's "Max depth m" field is the 3rd TextFormField
          // (Name, Description, Max depth).
          await $(TextField).at(2).enterText('30');
        },
      );
      await _waitForPin($, freeId);

      await _tapPin($, freeId);
      await $(freediving.FreedivingDetailSheet).waitUntilVisible();
      await $('Deep Cave').waitUntilVisible();

      // 4. Delete the freediving spot from its sheet.
      await $('Delete').tap();
      await $('Delete spot?').waitUntilVisible();
      // The confirmation dialog has Cancel + Delete; the last one is
      // the destructive action.
      await $('Delete').last.tap();
      await $(MainMapPage).waitUntilVisible();
      await _waitUntilGone($, freeId);

      // 5. The other spot (fishing) is still there.
      expect(find.byKey(Key('activity-pin-$fishId')), findsOneWidget);

      // 6. Restart the app (same prefs, fresh ProviderScope) — the
      //    user's saved place comes back from local cache + the
      //    server's read model.
      await harness.pumpTurboApp($.tester);
      await _waitForAppReady($);
      await _waitForPin($, fishId);
      expect(find.byKey(Key('activity-pin-$freeId')), findsNothing,
          reason: 'a deleted spot must not come back after a restart');
    },
  );

  patrolTest(
    'a fresh sign-up does not show me the previous owner\'s pin',
    config: const PatrolTesterConfig(visibleTimeout: Duration(seconds: 90)),
    ($) async {
      await harness.waitForBackendHealthy();
      await harness.resetAppState();

      // Another owner has a saved spot on the server. Their existence
      // is the precondition, not the thing under test — set them up
      // server-side and wait for the projector so the assertion below
      // reflects the owner-filter, not a propagation race.
      final owner = await harness.registerNewUser(tag: 'priv-owner');
      final ownerSpot = await _serverSideCreatePointSpot(
        accessToken: owner.accessToken,
        kindSlug: 'fishing',
        name: "Owner's secret",
        details: _fishingDefaults(),
      );
      await harness.waitForServerSidePlace(
          accessToken: owner.accessToken, named: "Owner's secret");

      // The thing under test: a new user signs up through the real UI
      // and looks at their map. The owner's pin must not be there.
      await harness.pumpTurboApp($.tester);
      await _waitForAppReady($);

      _pushScreen($, (_) => const RegisterScreen());
      await $(RegisterScreen).waitUntilVisible();
      await $(TextField).at(0).enterText(harness.uniqueEmail('priv-other'));
      await $(TextField).at(1).enterText('CorrectHorse-Battery-Staple-1!');
      await $(FilledButton).tap();
      await $(MainMapPage).waitUntilVisible();

      // Two summary-refresh windows worth of time.
      await Future<void>.delayed(const Duration(seconds: 5));
      expect(find.byKey(Key('activity-pin-$ownerSpot')), findsNothing,
          reason: "the owner's pin must never appear on a new user's map");
    },
  );

  patrolTest(
    'I can edit a saved fishing spot from its detail sheet',
    config: const PatrolTesterConfig(visibleTimeout: Duration(seconds: 60)),
    ($) async {
      await harness.waitForBackendHealthy();
      await harness.resetAppState();

      // The "I can sign up and save spots through the real UI" flow is
      // already covered exhaustively elsewhere in this suite. The
      // edit-specific user goal is "open my saved spot, change it,
      // see the new values" — set the spot up server-side and drive
      // only the edit half through the UI.
      final me = await harness.registerNewUser(tag: 'edit');
      final id = await _serverSideCreatePointSpot(
        accessToken: me.accessToken,
        kindSlug: 'fishing',
        name: 'Original name',
        details: _fishingDefaults(),
      );
      await harness.waitForServerSidePlace(
          accessToken: me.accessToken, named: 'Original name');
      await harness.seedAuthState(me);

      await harness.pumpTurboApp($.tester);
      await _waitForAppReady($);
      await _waitForPin($, id);

      // Open the detail sheet, tap Edit, change the name AND the
      // water-kind segmented selection, save.
      await _tapPin($, id);
      await $(fishing.FishingDetailSheet).waitUntilVisible();
      await $('Edit').tap();
      await $('Edit fishing spot').waitUntilVisible();
      await $(TextField).at(0).enterText('Renamed Fjord');
      await $('Sea').tap(); // change WaterKind segmented selection
      await $(FilledButton).scrollTo();
      await $(FilledButton).tap(); // Save
      await $(MainMapPage).waitUntilVisible();

      // The PUT goes through the outbox → projector path: the form
      // pops on 204, but the read model hasn't necessarily caught up
      // yet. Wait until the rename is visible before checking the UI
      // so a failure here means the UI didn't refresh, not that the
      // update is still propagating.
      final probe = harness.directApi()
        ..options.headers['Authorization'] = 'Bearer ${me.accessToken}';
      await _eventually(
        () async {
          final r = await probe.get('/api/activities/fishing/$id');
          return r.statusCode == 200 &&
              (r.data as Map)['name'] == 'Renamed Fjord' &&
              (((r.data as Map)['details']) as Map)['waterKind'] == 2;
        },
        reason: 'rename + water-kind change must be persisted server-side',
      );

      // Re-open the (same) pin and confirm the user sees the new
      // values on screen.
      await _tapPin($, id);
      await $('Renamed Fjord').waitUntilVisible();
      expect($('Sea'), findsOneWidget,
          reason: 'water-kind change should be persisted and displayed');
      expect($('Original name'), findsNothing,
          reason: 'old name should not be shown anywhere on the sheet');
    },
  );

  patrolTest(
    'I can sign out from the drawer and the sign-in screen comes back',
    config: const PatrolTesterConfig(visibleTimeout: Duration(seconds: 60)),
    ($) async {
      await harness.waitForBackendHealthy();
      await harness.resetAppState();

      final me = await harness.registerNewUser(tag: 'logout');
      await harness.seedAuthState(me);
      await harness.pumpTurboApp($.tester);
      await _waitForAppReady($);

      // Open the drawer via the menu icon on the search bar (the only
      // Icons.menu in the tree).
      await $.tester.tap(find.byIcon(Icons.menu));
      await $.pumpAndSettle();

      // The drawer's Logout entry. Tap it → drawer pops + confirm
      // dialog opens.
      await $('Logout').waitUntilVisible();
      await $('Logout').tap();
      // The dialog re-uses 'Logout' as its destructive action label;
      // `.last` targets the dialog button (drawer is already gone).
      await $('Logout').last.tap();

      // After logout, the menu's "Login" entry replaces "Logout".
      await $.pumpAndSettle();
      await $.tester.tap(find.byIcon(Icons.menu));
      await $('Login').waitUntilVisible();
    },
  );

  patrolTest(
    'I can promote a saved path to a hiking activity and see it on my map',
    config: const PatrolTesterConfig(visibleTimeout: Duration(seconds: 60)),
    ($) async {
      await harness.waitForBackendHealthy();
      await harness.resetAppState();

      // Sign in.
      final me = await harness.registerNewUser(tag: 'hike');
      await harness.seedAuthState(me);
      await harness.pumpTurboApp($.tester);
      await _waitForAppReady($);

      // Preconditions: the user has recorded / imported a path. We
      // can't drive a real GPS recording in the simulator, so we set
      // the saved path up via the same repository the recording flow
      // would call on save. From this point everything runs through
      // the real UI: the user opens the path's detail sheet, taps
      // "Save as activity", picks Hiking, fills the kind-specific
      // form, taps save, and the new hiking activity surfaces on the
      // map as a tappable pin.
      final ctx = $.tester.element(find.byType(MainMapPage));
      final container = ProviderScope.containerOf(ctx);
      final route = <LatLng>[
        const LatLng(60.420, 5.300),
        const LatLng(60.421, 5.305),
        const LatLng(60.422, 5.310),
        const LatLng(60.423, 5.315),
      ];
      final savedPath = paths.SavedPath(
        title: 'Stoltzekleiven',
        points: route,
        distance: 1200,
      );
      await container
          .read(paths.savedPathRepositoryProvider.notifier)
          .addPath(savedPath);

      // Open the path's detail sheet.
      _pushSheet(
        $,
        paths.PathDetailSheet(path: savedPath),
      );
      await $(paths.PathDetailSheet).waitUntilVisible();

      // Promote to an activity.
      await $('Save as activity').tap();
      // The cross-kind picker shows line-based kinds; Hiking is one.
      await $('Hiking').waitUntilVisible();
      await $('Hiking').tap();

      // The hiking create form. Required: Name. The route comes
      // pre-seeded from the saved path; the form's "Draw route"
      // button is optional (it'd re-open RouteDrawingScreen with
      // the saved path as a starting point).
      final beforeIds = await _currentActivityIds();
      await $(TextField).at(0).enterText('Up Stoltzen');
      await $(FilledButton).scrollTo();
      await $(FilledButton).tap();

      // HikingCreateScreen pops back to the PathDetailSheet (the
      // user's entry point), not the bare map — wait for the form
      // to be gone instead.
      await _waitUntilGoneByType($, 'HikingCreateScreen');

      // The new hiking activity is rendered on the map layer behind
      // the still-open PathDetailSheet. ActivitiesMapLayer keys the
      // marker by id, so the pin is findable in the widget tree even
      // though it's not hit-testable behind the sheet.
      final newId = await _waitForNewActivityId(beforeIds);
      await _waitForPin($, newId);

      // The user closes the path sheet via its X button (Icons.close).
      // Then they can tap the new hiking pin and see the name back.
      await $.tester.tap(find.byIcon(Icons.close));
      await $.pumpAndSettle();
      await _tapPin($, newId);
      await $('Up Stoltzen').waitUntilVisible();
    },
  );

  // The remaining line-based kinds share the saved-path → picker →
  // form → save → pin shape with hiking. Each test is short on
  // purpose: it proves the kind's create form renders, the picker
  // surfaces the kind tile for line geometry, and the new activity
  // shows up on the map. Anything kind-specific beyond the form's
  // required fields (name) is intentionally left to future tests
  // that target what's user-visibly distinct about that kind.

  patrolTest(
    'I can save a backcountry-ski tour from a saved path',
    config: const PatrolTesterConfig(visibleTimeout: Duration(seconds: 60)),
    ($) async => _runLineKindFromSavedPath($,
      tag: 'bcski',
      kindLabel: 'Backcountry skiing',
      activityName: 'Hardangerjøkulen',
      pathTitle: 'Tour route',
    ),
  );

  patrolTest(
    'I can save a cross-country ski tour from a saved path',
    config: const PatrolTesterConfig(visibleTimeout: Duration(seconds: 60)),
    ($) async => _runLineKindFromSavedPath($,
      tag: 'xcski',
      kindLabel: 'XC skiing',
      activityName: 'Nordmarka loop',
      pathTitle: 'Track from GPS',
    ),
  );

  patrolTest(
    'I can save a packrafting trip from a saved path',
    config: const PatrolTesterConfig(visibleTimeout: Duration(seconds: 60)),
    ($) async => _runLineKindFromSavedPath($,
      tag: 'packraft',
      kindLabel: 'Packrafting',
      activityName: 'Sjoa run',
      pathTitle: 'River line',
    ),
  );

  patrolTest(
    'I can open the route-drawing surface from a hiking form',
    config: const PatrolTesterConfig(visibleTimeout: Duration(seconds: 60)),
    ($) async {
      await harness.waitForBackendHealthy();
      await harness.resetAppState();

      final me = await harness.registerNewUser(tag: 'draw');
      await harness.seedAuthState(me);
      await harness.pumpTurboApp($.tester);
      await _waitForAppReady($);

      final ctx = $.tester.element(find.byType(MainMapPage));
      final container = ProviderScope.containerOf(ctx);
      final savedPath = paths.SavedPath(
        title: 'Draft route',
        points: const [
          LatLng(60.420, 5.300),
          LatLng(60.421, 5.305),
        ],
        distance: 600,
      );
      await container
          .read(paths.savedPathRepositoryProvider.notifier)
          .addPath(savedPath);

      _pushSheet($, paths.PathDetailSheet(path: savedPath));
      await $(paths.PathDetailSheet).waitUntilVisible();
      await $('Save as activity').tap();
      await $('Hiking').waitUntilVisible();
      await $('Hiking').tap();

      // Open the in-form route editor. The button sits below several
      // number fields in the ListView, so it has to be scrolled into
      // view before it's hit-testable.
      await $('Draw route on map').scrollTo();
      await $('Draw route on map').tap();

      // RouteDrawingScreen is reachable. Assert the user-facing
      // surface is shown — instruction text + Cancel + "Use route"
      // bottom-bar buttons.
      //
      // Why we stop here instead of driving the tap-to-add-vertex
      // flow: flutter_map's onTap doesn't fire from the synthetic
      // Flutter pointer events the test framework's `tapAt`
      // produces, and Patrol's native (XCUITest) tap kills the app
      // process on iOS Sim mid-test. The user-visible value "I can
      // open the route-drawing screen from the create form" is
      // covered; the actual map-tap → vertex flow is covered by
      // widget tests under `test/features/activities/`. See the
      // README's deliberate-non-test section.
      await $('Tap the map to add vertices.').waitUntilVisible();
      expect($('Cancel'), findsOneWidget);
      expect($('Use route'), findsOneWidget);
    },
  );

  patrolTest(
    'a wrong password tells me my password was wrong',
    config: const PatrolTesterConfig(visibleTimeout: Duration(seconds: 30)),
    ($) async {
      await harness.waitForBackendHealthy();
      await harness.resetAppState();

      final me = await harness.registerNewUser(tag: 'badpass');
      await harness.resetAppState(); // discard auto-login

      await harness.pumpTurboApp($.tester);
      await _waitForAppReady($);

      _pushScreen($, (_) => const LoginScreen());
      await $(LoginScreen).waitUntilVisible();

      await $(TextField).at(0).enterText(me.email);
      await $(TextField).at(1).enterText('definitely-not-my-password');
      await $(FilledButton).tap(); // "Sign in"

      await $(AuthErrorMessage).waitUntilVisible();
      expect($(LoginScreen), findsOneWidget,
          reason: 'login screen should remain open after a failed attempt');
    },
  );
}

// ── helpers ────────────────────────────────────────────────────────

/// Drives the real point-create flow: long-press the map → tap "Add
/// activity here" → tap the [kindLabel] in the picker → run [fillForm]
/// → scroll to + tap Save → return the new activity's server id so
/// callers can look up the pin by key.
Future<String> _createPointActivity(
  PatrolIntegrationTester $, {
  required String kindLabel,
  required Future<void> Function() fillForm,
}) async {
  final before = await _currentActivityIds();

  // Long-press the map at an explicit offset that's clear of the
  // overlay controls (search bar at the top, MapControls on the
  // right edge). Patrol's `$(FlutterMap).longPress()` resolves to
  // the widget's geometric centre which can occasionally collide
  // with a transparent overlay; using a hand-picked screen-relative
  // offset is more deterministic.
  final size = $.tester.view.physicalSize / $.tester.view.devicePixelRatio;
  await $.tester
      .longPressAt(Offset(size.width * 0.35, size.height * 0.55));
  await $.pumpAndSettle();
  await $('Add activity here').waitUntilVisible();
  await $('Add activity here').tap();
  await $(kindLabel).waitUntilVisible(); // picker tile
  await $(kindLabel).tap();
  await fillForm();
  // Longer forms (freediving) push Save below the iOS keyboard;
  // scroll first.
  await $(FilledButton).scrollTo();
  await $(FilledButton).tap();
  await $(MainMapPage).waitUntilVisible();

  // Wait for the server's read model to expose the new id — that's
  // the moment the pin can render via the next summary refresh.
  final deadline = DateTime.now().add(const Duration(seconds: 20));
  while (DateTime.now().isBefore(deadline)) {
    final added = (await _currentActivityIds()).difference(before);
    if (added.isNotEmpty) return added.first;
    await Future<void>.delayed(const Duration(milliseconds: 250));
  }
  fail('no new $kindLabel activity appeared on the server within 20s');
}

/// Drives the saved-path → ActivityCreatePicker → kind form → save
/// flow for one of the line-based kinds. Asserts the new pin appears
/// on the map and the saved activity's name shows in the kind's
/// detail sheet when the user taps the pin.
Future<void> _runLineKindFromSavedPath(
  PatrolIntegrationTester $, {
  required String tag,
  required String kindLabel,
  required String activityName,
  required String pathTitle,
}) async {
  await harness.waitForBackendHealthy();
  await harness.resetAppState();

  final me = await harness.registerNewUser(tag: tag);
  await harness.seedAuthState(me);
  await harness.pumpTurboApp($.tester);
  await _waitForAppReady($);

  final ctx = $.tester.element(find.byType(MainMapPage));
  final container = ProviderScope.containerOf(ctx);
  final savedPath = paths.SavedPath(
    title: pathTitle,
    points: const [
      LatLng(60.420, 5.300),
      LatLng(60.421, 5.305),
      LatLng(60.422, 5.310),
      LatLng(60.423, 5.315),
    ],
    distance: 1200,
  );
  await container
      .read(paths.savedPathRepositoryProvider.notifier)
      .addPath(savedPath);

  _pushSheet($, paths.PathDetailSheet(path: savedPath));
  await $(paths.PathDetailSheet).waitUntilVisible();
  await $('Save as activity').tap();
  await $(kindLabel).waitUntilVisible();
  await $(kindLabel).tap();

  final before = await _currentActivityIds();
  await $(TextField).at(0).enterText(activityName);
  await $(FilledButton).scrollTo();
  await $(FilledButton).tap();

  final newId = await _waitForNewActivityId(before);
  await _waitForPin($, newId);

  // Close the still-open PathDetailSheet, then tap the new pin and
  // assert the name the user typed is shown back to them.
  await $.tester.tap(find.byIcon(Icons.close));
  await $.pumpAndSettle();
  await _tapPin($, newId);
  await $(activityName).waitUntilVisible();
}

/// Server-side create for setup-only paths. The caller authenticates
/// as the owner via direct HTTP and posts a single point activity.
Future<String> _serverSideCreatePointSpot({
  required String accessToken,
  required String kindSlug,
  required String name,
  required Map<String, dynamic> details,
}) async {
  final dio = harness.directApi()
    ..options.headers['Authorization'] = 'Bearer $accessToken';
  final r = await dio.post('/api/activities/$kindSlug', data: {
    'name': name,
    'longitude': 5.322,
    'latitude': 60.391,
    'details': details,
  });
  if (r.statusCode != 201) {
    fail('server-side create failed ($kindSlug, $name): '
        '${r.statusCode} ${r.data}');
  }
  return (r.data as Map<String, dynamic>)['id'] as String;
}

Map<String, dynamic> _fishingDefaults() => {
      'waterKind': 1,
      'shoreOrBoat': 0,
      'targetSpecies': const [],
      'knownDepths': const [],
    };

Future<void> _tapPin(PatrolIntegrationTester $, String id) async {
  // The marker is wrapped in a GestureDetector; tapping it opens the
  // kind's detail sheet via showModalBottomSheet (see
  // ActivitiesMapLayer).
  await $.tester.tap(find.byKey(Key('activity-pin-$id')));
  await $.pump(const Duration(milliseconds: 500));
}

Future<void> _waitForPin(PatrolIntegrationTester $, String id) async {
  final deadline = DateTime.now().add(const Duration(seconds: 30));
  while (DateTime.now().isBefore(deadline)) {
    await $.pump(const Duration(milliseconds: 200));
    if (find.byKey(Key('activity-pin-$id')).evaluate().isNotEmpty) return;
  }
  fail('pin for activity $id never appeared on the map');
}

/// Polls [predicate] until it returns true or [timeout] elapses. Use
/// for server-state propagation, not for UI assertions (Patrol has
/// `waitUntilVisible` for that).
Future<void> _eventually(
  Future<bool> Function() predicate, {
  Duration timeout = const Duration(seconds: 15),
  Duration interval = const Duration(milliseconds: 250),
  required String reason,
}) async {
  final deadline = DateTime.now().add(timeout);
  while (DateTime.now().isBefore(deadline)) {
    if (await predicate()) return;
    await Future<void>.delayed(interval);
  }
  fail('eventually timed out after $timeout: $reason');
}

/// Waits until no widget whose runtimeType name equals [typeName] is in
/// the tree. Used when popping a screen and asserting it's gone (we
/// can't import non-exported screen types from a test file).
Future<void> _waitUntilGoneByType(
  PatrolIntegrationTester $,
  String typeName, {
  Duration timeout = const Duration(seconds: 30),
}) async {
  final deadline = DateTime.now().add(timeout);
  while (DateTime.now().isBefore(deadline)) {
    await $.pump(const Duration(milliseconds: 200));
    final any = $.tester.allWidgets
        .any((w) => w.runtimeType.toString() == typeName);
    if (!any) return;
  }
  fail('$typeName was still in the widget tree after $timeout');
}

Future<void> _waitUntilGone(PatrolIntegrationTester $, String id) async {
  final deadline = DateTime.now().add(const Duration(seconds: 15));
  while (DateTime.now().isBefore(deadline)) {
    await $.pump(const Duration(milliseconds: 200));
    if (find.byKey(Key('activity-pin-$id')).evaluate().isEmpty) return;
  }
  fail('pin for activity $id was still on the map after delete');
}

Future<void> _waitForAppReady(PatrolIntegrationTester $) async {
  final deadline = DateTime.now().add(const Duration(seconds: 15));
  while (DateTime.now().isBefore(deadline)) {
    await $.pump(const Duration(milliseconds: 100));
    final prefs = await SharedPreferences.getInstance();
    if (prefs.getBool('isLoggedIn') != null ||
        prefs.containsKey('accessToken') == false) {
      return;
    }
  }
}

void _pushScreen(
  PatrolIntegrationTester $,
  Widget Function(BuildContext) build,
) {
  final ctx = $.tester.element(find.byType(MainMapPage));
  Navigator.of(ctx).push(MaterialPageRoute(builder: build));
}

/// Shows a sheet via `showModalBottomSheet` — same way pin taps and
/// "show path detail" do in the live app.
void _pushSheet(PatrolIntegrationTester $, Widget sheet) {
  final ctx = $.tester.element(find.byType(MainMapPage));
  showModalBottomSheet<void>(
    context: ctx,
    isScrollControlled: true,
    useSafeArea: true,
    builder: (_) => sheet,
  );
}

/// Waits for a new activity id to appear server-side, beyond the set
/// the caller knew about before the action. Used after Save to learn
/// what id the projector assigned so the test can find the new pin.
Future<String> _waitForNewActivityId(Set<String> before) async {
  final deadline = DateTime.now().add(const Duration(seconds: 20));
  while (DateTime.now().isBefore(deadline)) {
    final added = (await _currentActivityIds()).difference(before);
    if (added.isNotEmpty) return added.first;
    await Future<void>.delayed(const Duration(milliseconds: 250));
  }
  fail('no new activity appeared on the server within 20s');
}

/// All activity ids currently visible to whatever user has tokens in
/// prefs. Used by `_createPointActivity` to identify the just-created
/// id without parsing the create response from the form's onTap path.
Future<Set<String>> _currentActivityIds() async {
  final prefs = await SharedPreferences.getInstance();
  final token = prefs.getString('accessToken');
  final dio = harness.directApi();
  if (token != null) dio.options.headers['Authorization'] = 'Bearer $token';
  final r = await dio.get('/api/activities/summaries/changes');
  if (r.statusCode != 200) return const {};
  final items = (r.data as Map?)?['items'] as List? ?? const [];
  return items.map((i) => (i as Map)['id'] as String).toSet();
}
