// End-to-end login user stories. Drives the real TurboApp against a real
// backend (gateway on $API_BASE_URL). Each test creates a unique user so
// the suite is idempotent and tests don't interact.
//
// Run with:
//   flutter test integration_test/login_flow_e2e_test.dart \
//     -d macos --dart-define=API_BASE_URL=http://localhost:8080

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
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
    'register a fresh user from the RegisterScreen ends in an authenticated session',
    (tester) async {
      await pumpTurboApp(tester);

      // Confirm we are in the unauthenticated cold-start state.
      final container = ProviderScope.containerOf(
        tester.element(find.byType(MainMapPage)),
      );
      expect(
        container.read(authStateProvider).status,
        AuthStatus.unauthenticated,
        reason: 'fresh prefs should land cold start at unauthenticated',
      );

      // Push the RegisterScreen directly (the same screen the "Create
      // account" link on LoginScreen lands on). Avoids navigating
      // through LoginScreen → tap link → RegisterScreen.
      // ignore: unawaited_futures
      RegisterScreen.show(tester.element(find.byType(MainMapPage)));
      await waitFor(tester, find.byType(RegisterScreen));

      await submitRegisterForm(
        tester,
        email: uniqueEmail('reg'),
        password: 'CorrectHorse-Battery-Staple-1!',
      );

      await _waitForAuthState(container, AuthStatus.authenticated);
      expect(find.byType(RegisterScreen), findsNothing);
    },
    timeout: const Timeout(Duration(seconds: 60)),
  );

  testWidgets(
    'login with valid credentials authenticates the session',
    (tester) async {
      // Pre-create a user via direct HTTP so we know valid credentials.
      final user = await registerNewUser(tag: 'login-ok');
      await resetAppState();

      await pumpTurboApp(tester);
      final container = ProviderScope.containerOf(
        tester.element(find.byType(MainMapPage)),
      );

      LoginScreen.show(tester.element(find.byType(MainMapPage)));
      await waitFor(tester, find.byType(LoginScreen));

      await submitLoginForm(
        tester,
        email: user.email,
        password: user.password,
      );

      await _waitForAuthState(container, AuthStatus.authenticated);
      expect(find.byType(LoginScreen), findsNothing);
    },
    timeout: const Timeout(Duration(seconds: 60)),
  );

  testWidgets(
    'login with wrong password surfaces an error and leaves the screen open',
    (tester) async {
      final user = await registerNewUser(tag: 'login-bad');
      await resetAppState();

      await pumpTurboApp(tester);
      final container = ProviderScope.containerOf(
        tester.element(find.byType(MainMapPage)),
      );

      LoginScreen.show(tester.element(find.byType(MainMapPage)));
      await waitFor(tester, find.byType(LoginScreen));

      await submitLoginForm(
        tester,
        email: user.email,
        password: 'wrong-password-12345',
      );

      // Give the API and notifier a moment to settle.
      await tester.pump(const Duration(seconds: 2));
      await pumpUntilSettled(tester);

      final state = container.read(authStateProvider);
      expect(
        state.status,
        isNot(AuthStatus.authenticated),
        reason: 'wrong password must not authenticate the session',
      );
      expect(find.byType(LoginScreen), findsOneWidget,
          reason: 'login screen should remain visible after a failure');
    },
    timeout: const Timeout(Duration(seconds: 60)),
  );

  testWidgets(
    'cold start with seeded tokens lands authenticated, logout returns to unauthenticated',
    (tester) async {
      final user = await registerNewUser(tag: 'logout');
      await seedAuthState(user);

      await pumpTurboApp(tester);
      final container = ProviderScope.containerOf(
        tester.element(find.byType(MainMapPage)),
      );
      await _waitForAuthState(container, AuthStatus.authenticated);

      // Drive logout directly through the notifier (avoids depending on
      // a specific settings/profile UI affordance).
      await container.read(authStateProvider.notifier).logout();
      await pumpUntilSettled(tester);

      expect(
        container.read(authStateProvider).status,
        AuthStatus.unauthenticated,
      );
    },
    timeout: const Timeout(Duration(seconds: 60)),
  );
}

/// Polls the auth state provider until it reaches [expected] or the
/// timeout elapses. Mirrors `waitForAsyncData` from test/helpers but
/// operates on the auth status enum.
Future<void> _waitForAuthState(
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
