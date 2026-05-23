// Patrol-driven user-journey tests. Run on iOS Simulator against a live
// docker-compose stack. Each test drives the *real* widgets and
// gestures the user uses — long-press on the map, pin-options sheet,
// activity picker, kind-specific form, text fields, segmented buttons,
// save — and asserts on what the user actually sees on screen
// (pins on the map, error banners, screen transitions).
//
// What's deliberately not tested:
//   - Editing an activity. The app has no edit UI; activities are
//     create-or-delete only. Adding a fake "edit via API" test would
//     verify implementation, not a user goal.
//   - Tapping a pin to open its detail sheet. `ActivityKindDescriptor.
//     buildDetailScreen` is defined on every kind but never wired to
//     the marker's onTap in the production UI (the marker is
//     non-interactive). Testing a sheet the user can't open verifies
//     code paths the user can't reach. Add the wire-up, then add the
//     test.
//   - Line-based kinds (hiking, backcountry-ski, xc-ski, packrafting).
//     They require a route-drawing UI; a single long-press on the
//     map cannot create them. The two point-based kinds (fishing,
//     freediving) are fully exercised.
//
// Run via the runner (which waits for /healthz first):
//   integration_test/run_e2e.sh

import 'package:flutter/material.dart';
import 'package:flutter_map/flutter_map.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:patrol/patrol.dart';
import 'package:shared_preferences/shared_preferences.dart';
import 'package:turbo/features/auth/api.dart';
import 'package:turbo/features/auth/widgets/auth_error_message.dart';
import 'package:turbo/features/map_view/api.dart';

import 'helpers/e2e_harness.dart' as harness;

void main() {
  patrolTest(
    'a new user signs up and saves two spots through the real map UI',
    config: const PatrolTesterConfig(visibleTimeout: Duration(seconds: 60)),
    ($) async {
      await harness.waitForBackendHealthy();
      await harness.resetAppState();
      await harness.pumpTurboApp($.tester);
      await _waitForAppReady($);

      // 1. Sign up through the real RegisterScreen.
      _pushScreen($, (_) => const RegisterScreen());
      await $(RegisterScreen).waitUntilVisible();
      await $(TextField).at(0).enterText(harness.uniqueEmail('signup'));
      await $(TextField).at(1).enterText('CorrectHorse-Battery-Staple-1!');
      await $(FilledButton).tap(); // "Create account"
      await $(MainMapPage).waitUntilVisible();

      // 2. Save a fishing spot via long-press → pin-options sheet →
      //    Add activity here → picker → Fishing → form → Save.
      final fishingId = await _createPointActivity(
        $,
        kindLabel: 'Fishing',
        fillForm: () async {
          await $(TextField).at(0).enterText('First Fjord');
          await $('Lake').tap();
          await $('Shore').tap();
        },
      );

      // The new pin renders on the user's map (the marker's key is
      // `activity-pin-$id`; see ActivitiesMapLayer).
      await _waitForPin($, fishingId);

      // 3. Save a second spot of a different kind so the user has
      //    multiple pins on their map. Freediving is the other
      //    point-based kind.
      final freeId = await _createPointActivity(
        $,
        kindLabel: 'Freediving',
        fillForm: () async {
          await $(TextField).at(0).enterText('Lysefjorden');
          // Freediving has an optional Description, then Max depth
          // (required, number). Skip description, set depth.
          // The "Max depth m" field is the 3rd TextFormField on the
          // form (Name, Description, Max depth).
          await $(TextField).at(2).enterText('30');
        },
      );
      await _waitForPin($, freeId);

      // The user now sees both of their saved spots on the map.
      expect(find.byKey(Key('activity-pin-$fishingId')), findsOneWidget);
      expect(find.byKey(Key('activity-pin-$freeId')), findsOneWidget);
    },
  );

  patrolTest(
    'returning to the app, both of my previously saved pins are on the map',
    config: const PatrolTesterConfig(visibleTimeout: Duration(seconds: 60)),
    ($) async {
      await harness.waitForBackendHealthy();
      await harness.resetAppState();

      // Set up two saved spots server-side as the prior session would
      // have done, then cold-start the app with the persisted tokens
      // — the same path a returning user takes.
      final me = await harness.registerNewUser(tag: 'returning');
      final spot1 = await _serverSideCreatePointSpot(
        accessToken: me.accessToken,
        kindSlug: 'fishing',
        name: 'Storevatnet',
        details: _fishingDefaults(),
      );
      final spot2 = await _serverSideCreatePointSpot(
        accessToken: me.accessToken,
        kindSlug: 'freediving',
        name: 'Vågen',
        details: _freedivingDefaults(),
      );
      await harness.waitForServerSidePlace(
          accessToken: me.accessToken, named: 'Storevatnet');
      await harness.waitForServerSidePlace(
          accessToken: me.accessToken, named: 'Vågen');
      await harness.seedAuthState(me);

      await harness.pumpTurboApp($.tester);
      await _waitForAppReady($);

      await _waitForPin($, spot1);
      await _waitForPin($, spot2);
    },
  );

  patrolTest(
    "another user's spots are not on my map",
    config: const PatrolTesterConfig(visibleTimeout: Duration(seconds: 60)),
    ($) async {
      await harness.waitForBackendHealthy();
      await harness.resetAppState();

      // Owner has a saved spot.
      final owner = await harness.registerNewUser(tag: 'priv-owner');
      final ownerSpot = await _serverSideCreatePointSpot(
        accessToken: owner.accessToken,
        kindSlug: 'fishing',
        name: 'Private Pond',
        details: _fishingDefaults(),
      );
      await harness.waitForServerSidePlace(
          accessToken: owner.accessToken, named: 'Private Pond');

      // Different user logs in fresh.
      final other = await harness.registerNewUser(tag: 'priv-other');
      await harness.seedAuthState(other);
      await harness.pumpTurboApp($.tester);
      await _waitForAppReady($);

      // Give the summaries repository two refreshes' worth of time to
      // pull anything it would pull. The owner's pin must never appear.
      await Future<void>.delayed(const Duration(seconds: 5));
      expect(find.byKey(Key('activity-pin-$ownerSpot')), findsNothing,
          reason: "the other user must not see the owner's pin on their map");
    },
  );

  // Note: an "offline tolerance" test was intentionally not included.
  // Patrol's `disableWifi` is Android-only — iOS Simulator doesn't
  // expose a Wi-Fi toggle XCUITest can flip. The user-visible
  // behaviour (pins served from local sqlite cache when the network
  // is gone) is covered by the activity_offline_behavior_test in the
  // widget-test suite. Add this back if/when Patrol supports iOS
  // Wi-Fi toggling or we move E2E to an Android emulator.

  patrolTest(
    'a wrong password tells me my password was wrong',
    config: const PatrolTesterConfig(visibleTimeout: Duration(seconds: 30)),
    ($) async {
      await harness.waitForBackendHealthy();
      await harness.resetAppState();

      final me = await harness.registerNewUser(tag: 'badpass');
      await harness.resetAppState(); // forget the auto-login

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
/// → tap Save → return the new activity's server id so callers can
/// look up the pin by key.
Future<String> _createPointActivity(
  PatrolIntegrationTester $, {
  required String kindLabel,
  required Future<void> Function() fillForm,
}) async {
  // The previously-known ids — used to find the new one after save.
  final before = await _currentActivityIds();

  await $(FlutterMap).longPress();
  // PinOptionsSheet's "Add activity here" tile is the user's path into
  // the picker; verify it surfaced.
  await $('Add activity here').waitUntilVisible();
  await $('Add activity here').tap();
  await $(kindLabel).waitUntilVisible(); // picker tile
  await $(kindLabel).tap();
  await fillForm();
  // The create forms render Save as a FilledButton; longer forms
  // (e.g. freediving) push it below the iOS keyboard / viewport, so
  // scroll it into view before tapping.
  await $(FilledButton).scrollTo();
  await $(FilledButton).tap();
  await $(MainMapPage).waitUntilVisible();

  // The save triggers an optimistic in-memory upsert + an outbox
  // event; the projector populates the read model. Wait until the
  // server has a new id we didn't see before.
  final deadline = DateTime.now().add(const Duration(seconds: 20));
  while (DateTime.now().isBefore(deadline)) {
    final now = await _currentActivityIds();
    final added = now.difference(before);
    if (added.isNotEmpty) return added.first;
    await Future<void>.delayed(const Duration(milliseconds: 250));
  }
  fail('no new $kindLabel activity appeared on the server within 20s');
}

Future<void> _waitForPin(PatrolIntegrationTester $, String id) async {
  // The pin is keyed by activity id (see ActivitiesMapLayer.build).
  // Polling on the widget tree lets us tolerate the time between the
  // save returning and the next summary refresh painting the pin.
  final deadline = DateTime.now().add(const Duration(seconds: 30));
  while (DateTime.now().isBefore(deadline)) {
    await $.pump(const Duration(milliseconds: 200));
    if (find.byKey(Key('activity-pin-$id')).evaluate().isNotEmpty) return;
  }
  fail('pin for activity $id never appeared on the map');
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

/// Current set of activity ids the signed-in user can see, fetched
/// straight from the gateway with whatever token's in prefs.
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

/// Server-side create for setup-only paths. Returns the new id.
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
      'waterKind': 1, // lake
      'shoreOrBoat': 0, // shore
      'targetSpecies': const [],
      'knownDepths': const [],
    };

Map<String, dynamic> _freedivingDefaults() => {
      'waterBody': 0, // sea
      'bottomType': 0,
      'maxDepthMeters': 25,
      'shoreEntry': true,
      'harpoonFishingAllowed': false,
    };
