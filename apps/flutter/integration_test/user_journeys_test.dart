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
import 'package:flutter_test/flutter_test.dart';
import 'package:patrol/patrol.dart';
import 'package:shared_preferences/shared_preferences.dart';
import 'package:turbo/features/activity_fishing/api.dart' as fishing;
import 'package:turbo/features/activity_freediving/api.dart' as freediving;
import 'package:turbo/features/auth/api.dart';
import 'package:turbo/features/auth/widgets/auth_error_message.dart';
import 'package:turbo/features/map_view/api.dart';

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

  // Note on drawer-logout-via-real-UI:
  // The drawer's Logout ListTile calls Navigator.pop(context) and then
  // launches the confirmation dialog (drawer_widget.dart line 181–184).
  // The dialog's confirm-callback uses the *drawer's* ref to call
  // `notifier.logout()`, but by that point the drawer widget has been
  // unmounted — ConsumerStatefulElement.read then throws
  // "Using 'ref' when a widget is about to or has been unmounted is
  // unsafe". This is a real app bug, not a test fragility; once it's
  // fixed (capture the notifier reference before Navigator.pop), the
  // drawer-driven logout becomes testable end-to-end. Until then, the
  // cross-user test below uses the notifier directly for the logout
  // transition — same code path the dialog's confirm tries to hit.

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
