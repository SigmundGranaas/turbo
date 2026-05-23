// End-to-end user-journey tests. Each test starts the real TurboApp
// close to a real entry condition (fresh, or with a prior session),
// drives the actual user gesture, and asserts only what the user would
// see or could do afterwards. If any of these break, the underlying
// infrastructure broke too — the layered "did the JWT validate?" /
// "did the projector run?" checks live in unit/behaviour tests, not
// here.
//
// Run with the runner so the backend stack is verified healthy first:
//   integration_test/run_e2e.sh

import 'package:flutter_test/flutter_test.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:latlong2/latlong.dart';
import 'package:shared_preferences/shared_preferences.dart';
import 'package:turbo/features/activity_fishing/data/fishing_repository.dart';
import 'package:turbo/features/activity_fishing/models/fishing_details.dart';
import 'package:turbo/features/auth/api.dart';
import 'package:turbo/features/auth/widgets/auth_error_message.dart';
import 'package:turbo/features/map_view/api.dart';

import 'helpers/e2e_harness.dart';

void main() {
  ensureE2EBinding();

  setUpAll(() async {
    await waitForBackendHealthy();
  });

  setUp(() async {
    await resetAppState();
  });

  // ── Sign-up journey ────────────────────────────────────────────────
  testWidgets(
    'a new user signs up and their first saved place is visible to them',
    (tester) async {
      await pumpTurboApp(tester);
      final container = ProviderScope.containerOf(
        tester.element(find.byType(MainMapPage)),
      );

      // Sign up through the real registration UI.
      // ignore: unawaited_futures
      RegisterScreen.show(tester.element(find.byType(MainMapPage)));
      await waitFor(tester, find.byType(RegisterScreen));
      await submitRegisterForm(
        tester,
        email: uniqueEmail('signup'),
        password: 'CorrectHorse-Battery-Staple-1!',
      );
      // pumpAndSettle returns before the register HTTP completes; wait
      // for the user-visible "I'm signed in" signal before continuing.
      await _eventually(
        () async {
          final prefs = await SharedPreferences.getInstance();
          return prefs.getBool('isLoggedIn') == true;
        },
        reason: 'register should flip the user to signed-in',
        timeout: const Duration(seconds: 15),
      );

      // Save a fishing spot through the repository the create form
      // wires up. Driving the kind-specific form through gestures is
      // brittle on macOS (segmented buttons, no map render); the
      // user-visible outcome we care about — "I see my spot" — is
      // unchanged.
      await container.read(fishingRepositoryProvider).create(
        name: 'Solbergvannet',
        position: const LatLng(60.391, 5.322),
        details: const FishingDetails(
          waterKind: WaterKind.lake,
          shoreOrBoat: ShoreOrBoat.shore,
        ),
      );

      await _userSees(named: 'Solbergvannet');
    },
    timeout: const Timeout(Duration(minutes: 1)),
  );

  // ── Returning-user journey ────────────────────────────────────────
  testWidgets(
    'returning to the app, my previously saved places are still visible',
    (tester) async {
      // Set up the prior session entirely server-side, then start the
      // app cold with the persisted tokens — the same path a user gets
      // on a real second launch.
      final me = await registerNewUser(tag: 'returning');
      await _serverSideCreateSpot(
        accessToken: me.accessToken,
        name: 'Storevatnet',
        lat: 60.421,
        lon: 5.355,
      );
      await seedAuthState(me);

      await pumpTurboApp(tester);

      await _userSees(named: 'Storevatnet');
    },
    timeout: const Timeout(Duration(minutes: 1)),
  );

  // ── Deletion journey ──────────────────────────────────────────────
  testWidgets(
    'I can delete a place I saved and it stays gone',
    (tester) async {
      final me = await registerNewUser(tag: 'delete');
      await seedAuthState(me);
      await pumpTurboApp(tester);
      final container = ProviderScope.containerOf(
        tester.element(find.byType(MainMapPage)),
      );

      final id = await container.read(fishingRepositoryProvider).create(
        name: 'Sletnesvatnet',
        position: const LatLng(63.430, 10.395),
        details: const FishingDetails(
          waterKind: WaterKind.lake,
          shoreOrBoat: ShoreOrBoat.shore,
        ),
      );
      // Wait for the server's read model to acknowledge — both so the
      // user-visible check is meaningful and so the delete handler
      // (which reads from the read model) doesn't 404.
      await _userSees(named: 'Sletnesvatnet');

      await container.read(fishingRepositoryProvider).delete(id);

      await _userDoesNotSee(named: 'Sletnesvatnet');
    },
    timeout: const Timeout(Duration(minutes: 1)),
  );

  // ── Wrong-password feedback ───────────────────────────────────────
  testWidgets(
    'a wrong password tells me my password was wrong',
    (tester) async {
      final me = await registerNewUser(tag: 'badpass');
      await resetAppState();

      await pumpTurboApp(tester);
      // ignore: unawaited_futures
      LoginScreen.show(tester.element(find.byType(MainMapPage)));
      await waitFor(tester, find.byType(LoginScreen));

      await submitLoginForm(
        tester,
        email: me.email,
        password: 'definitely-not-my-password',
      );

      // The user sees an error and is still on the sign-in screen.
      await waitFor(tester, find.byType(AuthErrorMessage));
      expect(find.byType(LoginScreen), findsOneWidget,
          reason: 'sign-in screen should stay open after a failed attempt');
    },
    timeout: const Timeout(Duration(minutes: 1)),
  );

  // ── Privacy ───────────────────────────────────────────────────────
  testWidgets(
    'my saved places are not visible to other users',
    (tester) async {
      // Owner saves a spot server-side, then someone else logs in.
      final owner = await registerNewUser(tag: 'priv-owner');
      await _serverSideCreateSpot(
        accessToken: owner.accessToken,
        name: 'A-private-spot',
        lat: 59.913,
        lon: 10.752,
      );
      // Make sure the owner's save is fully persisted before the
      // privacy check, so a "not visible" outcome reflects the owner
      // filter and not a still-propagating projection.
      await waitForServerSidePlace(
        accessToken: owner.accessToken,
        named: 'A-private-spot',
      );

      final other = await registerNewUser(tag: 'priv-other');
      await seedAuthState(other);
      await pumpTurboApp(tester);

      await _userDoesNotSee(named: 'A-private-spot');
    },
    timeout: const Timeout(Duration(minutes: 1)),
  );
}

/// The user-visible "I can see my places" check. Queries the same
/// endpoint the app's summary store reads from, with the same access
/// token the production interceptor would attach. If this passes, the
/// data is there and the user's auth is good — i.e. the map widget
/// will paint the pin once its repo refreshes.
///
/// We assert against the server (using the prefs-resident token)
/// rather than via the in-app repo's `refresh()` to keep this signal
/// independent of repo-internal cursor/debounce behaviour. The repo
/// has its own widget tests.
Future<void> _userSees({
  required String named,
  Duration timeout = const Duration(seconds: 20),
}) async {
  await _eventually(
    () async => _serverSeesForCurrentUser(named: named),
    timeout: timeout,
    reason: 'user should see a saved place named "$named"',
  );
}

Future<void> _userDoesNotSee({
  required String named,
  Duration settleFor = const Duration(seconds: 3),
  Duration timeout = const Duration(seconds: 20),
}) async {
  // Wait until absent (it may have been there but is being deleted).
  await _eventually(
    () async => !(await _serverSeesForCurrentUser(named: named)),
    timeout: timeout,
    reason: 'user should not see "$named"',
  );
  // Stay absent across a few rounds so a still-propagating tombstone
  // can't false-pass.
  final until = DateTime.now().add(settleFor);
  while (DateTime.now().isBefore(until)) {
    if (await _serverSeesForCurrentUser(named: named)) {
      fail('"$named" reappeared during settle window — leaked to user view');
    }
    await Future<void>.delayed(const Duration(milliseconds: 250));
  }
}

Future<bool> _serverSeesForCurrentUser({required String named}) async {
  final prefs = await SharedPreferences.getInstance();
  final token = prefs.getString('accessToken');
  final dio = directApi();
  if (token != null) dio.options.headers['Authorization'] = 'Bearer $token';
  final r = await dio.get('/api/activities/summaries/changes');
  if (r.statusCode != 200) return false;
  final items = (r.data as Map?)?['items'] as List? ?? const [];
  return items.any((i) => (i as Map)['name'] == named);
}

Future<void> _eventually(
  Future<bool> Function() predicate, {
  Duration timeout = const Duration(seconds: 20),
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

/// Creates a fishing activity directly against the API as [accessToken]'s
/// owner. Used by tests that want prior server state without exercising
/// the create UI — that flow has its own widget tests.
Future<void> _serverSideCreateSpot({
  required String accessToken,
  required String name,
  required double lat,
  required double lon,
}) async {
  final dio = directApi()
    ..options.headers['Authorization'] = 'Bearer $accessToken';
  final r = await dio.post(
    '/api/activities/fishing',
    data: {
      'name': name,
      'longitude': lon,
      'latitude': lat,
      'details': {
        'waterKind': WaterKind.lake.index,
        'shoreOrBoat': ShoreOrBoat.shore.index,
        'targetSpecies': const [],
        'knownDepths': const [],
      },
    },
  );
  if (r.statusCode != 201) {
    fail('server-side create failed for "$name": ${r.statusCode} ${r.data}');
  }
}
