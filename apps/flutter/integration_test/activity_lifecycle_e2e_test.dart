// End-to-end activity lifecycle user stories. Drives the real TurboApp
// against a real backend stack and exercises the full create → projection →
// summaries delta loop, then delete → tombstone loop. Crosses every layer
// the UI does: auth interceptor + Dio + gateway + activity host + Postgres +
// in-process projection (modulith) or NATS subscriber (microservices).
//
// Run with:
//   flutter test integration_test/activity_lifecycle_e2e_test.dart \
//     -d macos --dart-define=API_BASE_URL=http://localhost:8080

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:latlong2/latlong.dart';
import 'package:turbo/features/activities/api.dart' as activities;
import 'package:turbo/features/activity_fishing/api.dart' as activity_fishing;
import 'package:turbo/features/activity_fishing/data/fishing_repository.dart';
import 'package:turbo/features/activity_fishing/models/fishing_details.dart';
import 'package:turbo/features/auth/api.dart';
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

  testWidgets(
    'authenticated user creates a fishing activity → it appears in the cross-kind summaries',
    (tester) async {
      final user = await registerNewUser(tag: 'fish-create');
      await seedAuthState(user);

      await pumpTurboApp(tester);
      final container = ProviderScope.containerOf(
        tester.element(find.byType(MainMapPage)),
      );
      await _waitForAuthStatus(container, AuthStatus.authenticated);

      // Drive the create through the same repository the UI uses, so we
      // exercise the real ApiClient + JWT interceptor + endpoint contract.
      const spot = LatLng(60.391, 5.322); // Bergen-ish
      final repo = container.read(fishingRepositoryProvider);
      final id = await repo.create(
        name: 'E2E test spot',
        description: 'Created by integration test',
        position: spot,
        details: const FishingDetails(
          waterKind: WaterKind.lake,
          shoreOrBoat: ShoreOrBoat.shore,
          accessNotes: 'short walk from the trail',
        ),
      );
      expect(id, isNotEmpty,
          reason: 'POST /api/activities/fishing must return an id');

      // After create() the in-memory store carries an optimistic upsert
      // immediately, no delta-round-trip required.
      final summariesNotifier = container
          .read(activities.activitySummariesRepositoryProvider.notifier);
      expect(
        container.read(activities.activitySummariesRepositoryProvider).value?[id]?.name,
        'E2E test spot',
        reason: 'optimistic upsert should land in the summary store',
      );

      // Force a real delta refresh — this round-trips through the server
      // projection / NATS subscriber and tests the wire format. The
      // create handler publishes two events through the outbox (fishing
      // detail + cross-kind summary) that are projected asynchronously,
      // so a few rounds may be needed.
      await _eventually(() async {
        await summariesNotifier.refresh();
        final summary = container
            .read(activities.activitySummariesRepositoryProvider)
            .value?[id];
        return summary != null && summary.name == 'E2E test spot';
      }, reason: 'server projection should surface the new activity in summaries');

      // The typed detail read goes through a separate projector consuming
      // the same outbox; it may settle a moment after the summary.
      final api = container.read(activity_fishing.fishingApiProvider);
      final detail = await _eventuallyValue(
        () => api.getById(id),
        reason: 'fishing detail projector should catch up after create',
      );
      expect(detail.name, 'E2E test spot');
      expect(detail.details.waterKind, WaterKind.lake);
      expect(detail.details.shoreOrBoat, ShoreOrBoat.shore);
      expect(detail.details.accessNotes, 'short walk from the trail');
    },
    timeout: const Timeout(Duration(seconds: 90)),
  );

  testWidgets(
    'deleting an activity removes it from the summaries and from the detail endpoint',
    (tester) async {
      final user = await registerNewUser(tag: 'fish-delete');
      await seedAuthState(user);

      await pumpTurboApp(tester);
      final container = ProviderScope.containerOf(
        tester.element(find.byType(MainMapPage)),
      );
      await _waitForAuthStatus(container, AuthStatus.authenticated);

      final repo = container.read(fishingRepositoryProvider);
      final api = container.read(activity_fishing.fishingApiProvider);
      final id = await repo.create(
        name: 'soon-to-be-deleted spot',
        position: const LatLng(63.430, 10.395),
        details: const FishingDetails(
          waterKind: WaterKind.sea,
          shoreOrBoat: ShoreOrBoat.boat,
        ),
      );
      // Wait until the server's read model has caught up (both projectors
      // need to settle: fishing detail and cross-kind summary).
      final notifier = container
          .read(activities.activitySummariesRepositoryProvider.notifier);
      await _eventually(() async {
        await notifier.refresh();
        return container
                .read(activities.activitySummariesRepositoryProvider)
                .value?[id] !=
            null;
      }, reason: 'create should be reachable via summaries before delete');
      await _eventuallyValue(
        () => api.getById(id),
        reason: 'fishing detail must exist before delete',
      );

      await repo.delete(id);

      // Local optimistic removal first.
      expect(
        container.read(activities.activitySummariesRepositoryProvider).value?[id],
        isNull,
        reason: 'delete should immediately drop the row from the local store',
      );

      // Then verify the server tombstone propagates: refresh sees it
      // removed AND the detail endpoint reports 404. Both projectors
      // again settle independently.
      await _eventually(() async {
        await notifier.refresh();
        return container
                .read(activities.activitySummariesRepositoryProvider)
                .value?[id] ==
            null;
      }, reason: 'server tombstone should propagate to summaries');
      await _eventually(() async {
        try {
          await api.getById(id);
          return false;
        } catch (_) {
          return true;
        }
      }, reason: 'GET /api/activities/fishing/{deleted-id} should 404');
    },
    timeout: const Timeout(Duration(seconds: 90)),
  );

  testWidgets(
    'a user cannot read another user\'s activity detail',
    (tester) async {
      // User A creates an activity.
      final userA = await registerNewUser(tag: 'iso-a');
      await seedAuthState(userA);
      await pumpTurboApp(tester);
      final container = ProviderScope.containerOf(
        tester.element(find.byType(MainMapPage)),
      );
      await _waitForAuthStatus(container, AuthStatus.authenticated);

      final repoA = container.read(fishingRepositoryProvider);
      final id = await repoA.create(
        name: 'private spot',
        position: const LatLng(59.913, 10.752),
        details: const FishingDetails(
          waterKind: WaterKind.river,
          shoreOrBoat: ShoreOrBoat.shore,
        ),
      );
      // Ensure A's create is visible server-side before swapping users
      // so we know any later 404 isn't just a projection race.
      await _eventuallyValue(
        () => container.read(activity_fishing.fishingApiProvider).getById(id),
        reason: 'A should see their own activity before user switch',
      );

      // Switch to user B in the same container: log out (drops A's
      // tokens + auth state) then log in as B (writes B's tokens to
      // SharedPreferences via the production flow). The same ApiClient
      // / interceptors are reused, exactly like a real user-switch in
      // the live app.
      final userB = await registerNewUser(tag: 'iso-b');
      await container.read(authStateProvider.notifier).logout();
      await _waitForAuthStatus(container, AuthStatus.unauthenticated);
      await container
          .read(authStateProvider.notifier)
          .login(userB.email, userB.password);
      await _waitForAuthStatus(container, AuthStatus.authenticated);

      // Clear B's local summary cache so a stale row from A's session
      // doesn't masquerade as a server-leak. The repository keeps an
      // in-memory map across login/logout in the real app, but a real
      // user-switch should never have A's pins visible to B.
      final notifierLocal = container
          .read(activities.activitySummariesRepositoryProvider.notifier);
      notifierLocal.removeLocal(id);

      // B should not see A's activity via summaries — even after a real
      // refresh (the projector has had ample time by now). Poll a few
      // times to make sure the absence is stable, not just a transient.
      for (var i = 0; i < 5; i++) {
        await notifierLocal.refresh();
        await Future<void>.delayed(const Duration(milliseconds: 250));
      }
      expect(
        container.read(activities.activitySummariesRepositoryProvider).value?[id],
        isNull,
        reason: 'cross-user summaries must not leak ownership',
      );

      // And B's direct detail read must fail (the row exists for A but
      // the controller filters on OwnerId == userId).
      final api = container.read(activity_fishing.fishingApiProvider);
      await expectLater(
        api.getById(id),
        throwsA(isA<Object>()),
        reason: 'detail endpoint must reject cross-user reads',
      );
    },
    timeout: const Timeout(Duration(seconds: 120)),
  );
}

/// Retries [predicate] on a polling schedule until it returns true or the
/// timeout elapses. Adapted from `wait_for.dart` patterns used in widget
/// tests but works against any async boolean predicate.
Future<void> _eventually(
  Future<bool> Function() predicate, {
  Duration timeout = const Duration(seconds: 15),
  Duration interval = const Duration(milliseconds: 250),
  String? reason,
}) async {
  final deadline = DateTime.now().add(timeout);
  while (DateTime.now().isBefore(deadline)) {
    if (await predicate()) return;
    await Future<void>.delayed(interval);
  }
  fail('eventually timed out after $timeout${reason == null ? '' : ': $reason'}');
}

/// Like [_eventually] but expects [thunk] to *return* a value instead of
/// throwing. The thunk is retried on any exception until it returns or
/// the timeout elapses. Useful for GETs against an eventually-consistent
/// read model that initially 404s while a projector is catching up.
Future<T> _eventuallyValue<T>(
  Future<T> Function() thunk, {
  Duration timeout = const Duration(seconds: 15),
  Duration interval = const Duration(milliseconds: 250),
  String? reason,
}) async {
  final deadline = DateTime.now().add(timeout);
  Object? lastError;
  while (DateTime.now().isBefore(deadline)) {
    try {
      return await thunk();
    } catch (e) {
      lastError = e;
    }
    await Future<void>.delayed(interval);
  }
  fail('eventuallyValue timed out after $timeout '
      '(last error: $lastError)${reason == null ? '' : ': $reason'}');
}

Future<void> _waitForAuthStatus(
  ProviderContainer container,
  AuthStatus expected, {
  Duration timeout = const Duration(seconds: 15),
}) async {
  final deadline = DateTime.now().add(timeout);
  while (DateTime.now().isBefore(deadline)) {
    if (container.read(authStateProvider).status == expected) return;
    await Future<void>.delayed(const Duration(milliseconds: 100));
  }
  fail('Auth state did not reach $expected within $timeout '
      '(last: ${container.read(authStateProvider).status})');
}
