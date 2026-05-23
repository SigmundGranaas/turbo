// Patrol-driven user-journey tests. Run on iOS Simulator against a live
// docker-compose stack. Each test drives the *real* widgets the user
// touches — fields, segmented buttons, save buttons, error banners,
// detail sheets, confirmation dialogs — and asserts on what the user
// actually sees on screen.
//
// Map-tap interactions (long-press the map → picker → kind form, tap
// a pin → detail sheet) require a reliable Marker locator on MapLibre.
// Until that exists we push the screens via the Navigator. The form
// and sheet behaviour the user sees is identical.
//
// Run via the runner (which waits for /healthz first):
//   integration_test/run_e2e.sh

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:latlong2/latlong.dart';
import 'package:patrol/patrol.dart';
import 'package:shared_preferences/shared_preferences.dart';
import 'package:turbo/features/activities/api.dart' as activities;
import 'package:turbo/features/activity_fishing/api.dart' as fishing;
import 'package:turbo/features/activity_fishing/widgets/fishing_detail_sheet.dart';
import 'package:turbo/features/auth/api.dart';
import 'package:turbo/features/auth/widgets/auth_error_message.dart';
import 'package:turbo/features/map_view/api.dart';

import 'helpers/e2e_harness.dart' as harness;

void main() {
  patrolTest(
    'a new user signs up, saves a fishing spot, and sees it on their map',
    config: const PatrolTesterConfig(visibleTimeout: Duration(seconds: 30)),
    ($) async {
      await harness.waitForBackendHealthy();
      await harness.resetAppState();
      await harness.pumpTurboApp($.tester);
      await _waitForAppReady($);

      _pushScreen($, (_) => const RegisterScreen());
      await $(RegisterScreen).waitUntilVisible();

      final email = harness.uniqueEmail('signup');
      await $(TextField).at(0).enterText(email);
      await $(TextField).at(1).enterText('CorrectHorse-Battery-Staple-1!');
      await $(FilledButton).tap(); // 'Create account' button

      await $(MainMapPage).waitUntilVisible();

      _pushScreen(
        $,
        (_) => fishing.FishingCreateScreen(
          seedGeometry: activities.ActivityGeometry.fromPoint(
            const LatLng(60.391, 5.322),
          ),
        ),
      );
      await $(fishing.FishingCreateScreen).waitUntilVisible();

      await $(TextField).at(0).enterText('Solbergvannet');
      await $('Lake').tap();
      await $('Shore').tap();
      await $('Save').tap();

      await $(MainMapPage).waitUntilVisible();

      // The user can re-open the activity and see the data they
      // entered — this is what "saved" means to them.
      final id = await _findActivityId(named: 'Solbergvannet');
      _pushSheet($, FishingDetailSheet(activityId: id));
      await $('Solbergvannet').waitUntilVisible();
      expect($('Lake'), findsOneWidget,
          reason: 'detail sheet should show the water type the user chose');
      expect($('Shore'), findsOneWidget,
          reason: 'detail sheet should show the access type the user chose');
    },
  );

  patrolTest(
    'returning to the app, my saved spot is still there to see',
    config: const PatrolTesterConfig(visibleTimeout: Duration(seconds: 30)),
    ($) async {
      await harness.waitForBackendHealthy();
      await harness.resetAppState();

      // Prior session, fully server-side. Wait for projection so the
      // detail endpoint can serve the row on cold start.
      final me = await harness.registerNewUser(tag: 'returning');
      await _serverSideCreateSpot(
          accessToken: me.accessToken, name: 'Storevatnet');
      await harness.waitForServerSidePlace(
          accessToken: me.accessToken, named: 'Storevatnet');
      await harness.seedAuthState(me);

      await harness.pumpTurboApp($.tester);
      await _waitForAppReady($);

      // After cold start, the user opens their saved spot and sees it.
      final id = await _findActivityId(named: 'Storevatnet');
      _pushSheet($, FishingDetailSheet(activityId: id));
      await $('Storevatnet').waitUntilVisible();
    },
  );

  patrolTest(
    'I can delete a spot from its detail sheet and it stays gone',
    config: const PatrolTesterConfig(visibleTimeout: Duration(seconds: 30)),
    ($) async {
      await harness.waitForBackendHealthy();
      await harness.resetAppState();

      final me = await harness.registerNewUser(tag: 'delete');
      await _serverSideCreateSpot(
          accessToken: me.accessToken, name: 'Sletnesvatnet');
      await harness.waitForServerSidePlace(
          accessToken: me.accessToken, named: 'Sletnesvatnet');
      await harness.seedAuthState(me);

      await harness.pumpTurboApp($.tester);
      await _waitForAppReady($);

      final id = await _findActivityId(named: 'Sletnesvatnet');
      _pushSheet($, FishingDetailSheet(activityId: id));
      await $('Sletnesvatnet').waitUntilVisible();

      // Tap delete → confirm dialog → confirm.
      await $('Delete').tap();
      await $('Delete spot?').waitUntilVisible();
      // The dialog has Cancel + Delete; tap the destructive one.
      await $('Delete').last.tap();

      // Sheet pops; we're back at the map. The server tombstone is in
      // flight, but the next attempt to look at the spot must fail.
      await $(MainMapPage).waitUntilVisible();
      await _waitUntilServerSideGone(
          accessToken: me.accessToken, named: 'Sletnesvatnet');
    },
  );

  patrolTest(
    "another user cannot see my private spot in their detail view",
    config: const PatrolTesterConfig(visibleTimeout: Duration(seconds: 30)),
    ($) async {
      await harness.waitForBackendHealthy();
      await harness.resetAppState();

      final owner = await harness.registerNewUser(tag: 'priv-owner');
      await _serverSideCreateSpot(
          accessToken: owner.accessToken, name: 'A-private-spot');
      await harness.waitForServerSidePlace(
          accessToken: owner.accessToken, named: 'A-private-spot');
      final ownerSpotId = await _findActivityId(
          accessToken: owner.accessToken, named: 'A-private-spot');

      final other = await harness.registerNewUser(tag: 'priv-other');
      await harness.seedAuthState(other);

      await harness.pumpTurboApp($.tester);
      await _waitForAppReady($);

      _pushSheet($, FishingDetailSheet(activityId: ownerSpotId));
      // Give the provider time to settle into either loading, error,
      // or — in the impossible-but-this-is-what-we're-checking — data.
      await $.pump(const Duration(seconds: 3));
      // The owner's name must never appear on the other user's screen.
      // This is what "private" means to the user; the specific
      // failure mode (loading spinner, error banner, empty) doesn't
      // matter as long as their data doesn't leak.
      expect($('A-private-spot'), findsNothing,
          reason: "the other user must not see the owner's data anywhere");
    },
  );

  patrolTest(
    'a wrong password tells me my password was wrong',
    config: const PatrolTesterConfig(visibleTimeout: Duration(seconds: 30)),
    ($) async {
      await harness.waitForBackendHealthy();
      await harness.resetAppState();

      final me = await harness.registerNewUser(tag: 'badpass');
      // Reset prefs so the auth init doesn't auto-log us in.
      await harness.resetAppState();

      await harness.pumpTurboApp($.tester);
      await _waitForAppReady($);

      _pushScreen($, (_) => const LoginScreen());
      await $(LoginScreen).waitUntilVisible();

      await $(TextField).at(0).enterText(me.email);
      await $(TextField).at(1).enterText('definitely-not-my-password');
      await $(FilledButton).tap(); // 'Sign in' button

      await $(AuthErrorMessage).waitUntilVisible();
      expect($(LoginScreen), findsOneWidget,
          reason: 'login screen should remain open after a failed attempt');
    },
  );
}

// ── helpers ────────────────────────────────────────────────────────

/// Polls SharedPreferences until the cold-start auth gate has settled.
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

/// Pushes a [MaterialPageRoute] onto the live app's navigator.
void _pushScreen(
  PatrolIntegrationTester $,
  Widget Function(BuildContext) build,
) {
  final ctx = $.tester.element(find.byType(MainMapPage));
  Navigator.of(ctx).push(MaterialPageRoute(builder: build));
}

/// Shows a sheet widget via showModalBottomSheet — the same way pin
/// taps surface detail sheets in the live app.
void _pushSheet(PatrolIntegrationTester $, Widget sheet) {
  final ctx = $.tester.element(find.byType(MainMapPage));
  showModalBottomSheet(
    context: ctx,
    builder: (_) => sheet,
    isScrollControlled: true,
  );
}

/// Server-side fishing create using the supplied token. Used by tests
/// that want prior server state without exercising the create UI.
Future<void> _serverSideCreateSpot({
  required String accessToken,
  required String name,
}) async {
  final dio = harness.directApi()
    ..options.headers['Authorization'] = 'Bearer $accessToken';
  final r = await dio.post(
    '/api/activities/fishing',
    data: {
      'name': name,
      'longitude': 5.322,
      'latitude': 60.391,
      'details': {
        'waterKind': 1, // lake
        'shoreOrBoat': 0, // shore
        'targetSpecies': const [],
        'knownDepths': const [],
      },
    },
  );
  if (r.statusCode != 201) {
    fail('server-side create failed for "$name": ${r.statusCode} ${r.data}');
  }
}

/// Finds the id of the most-recently-projected activity with the
/// given name for the caller (token defaults to whatever's in prefs).
/// Used to push the detail sheet, which is keyed on activity id.
Future<String> _findActivityId({
  required String named,
  String? accessToken,
}) async {
  final token =
      accessToken ?? (await SharedPreferences.getInstance()).getString('accessToken');
  if (token == null) fail('no access token available to look up "$named"');
  final dio = harness.directApi()
    ..options.headers['Authorization'] = 'Bearer $token';
  // Poll briefly — the projection may still be in flight if the caller
  // didn't wait first.
  final deadline = DateTime.now().add(const Duration(seconds: 10));
  while (DateTime.now().isBefore(deadline)) {
    final r = await dio.get('/api/activities/summaries/changes');
    final items = (r.data as Map?)?['items'] as List? ?? const [];
    final hit = items.firstWhere(
      (i) => (i as Map)['name'] == named,
      orElse: () => null,
    );
    if (hit != null) return (hit as Map)['id'] as String;
    await Future<void>.delayed(const Duration(milliseconds: 250));
  }
  fail('no activity named "$named" visible to this user within 10s');
}

Future<void> _waitUntilServerSideGone({
  required String accessToken,
  required String named,
  Duration timeout = const Duration(seconds: 15),
}) async {
  final dio = harness.directApi()
    ..options.headers['Authorization'] = 'Bearer $accessToken';
  final deadline = DateTime.now().add(timeout);
  while (DateTime.now().isBefore(deadline)) {
    final r = await dio.get('/api/activities/summaries/changes');
    final items = (r.data as Map?)?['items'] as List? ?? const [];
    if (!items.any((i) => (i as Map)['name'] == named)) return;
    await Future<void>.delayed(const Duration(milliseconds: 250));
  }
  fail('server still shows "$named" after $timeout');
}
