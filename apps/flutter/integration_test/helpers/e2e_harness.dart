// End-to-end test harness: boots the real TurboApp against a real backend
// (the local docker compose stack) and exposes small, focused helpers for
// user-story tests.
//
// Conventions:
//  - The backend URL is passed via `--dart-define=API_BASE_URL=...` and
//    consumed by `EnvironmentConfig.apiBaseUrl`. Default is the gateway
//    URL the compose stack exposes (`http://localhost:8080`).
//  - Each test creates its own user via `uniqueEmail()` so tests are
//    independent and the suite is idempotent without DB resets.
//  - SharedPreferences are reset before every test so cold-start auth
//    gating reliably starts at unauthenticated.

import 'package:dio/dio.dart';
import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:integration_test/integration_test.dart';
import 'package:shared_preferences/shared_preferences.dart';
import 'package:turbo/app/app.dart';
import 'package:turbo/core/widgets/app_button.dart';
import 'package:turbo/features/activities/api.dart' as activities;
import 'package:turbo/features/activity_backcountry_ski/api.dart'
    as activity_backcountry_ski;
import 'package:turbo/features/activity_fishing/api.dart' as activity_fishing;
import 'package:turbo/features/activity_freediving/api.dart'
    as activity_freediving;
import 'package:turbo/features/activity_hiking/api.dart' as activity_hiking;
import 'package:turbo/features/activity_packrafting/api.dart'
    as activity_packrafting;
import 'package:turbo/features/activity_xc_ski/api.dart' as activity_xc_ski;
import 'package:flutter_riverpod/flutter_riverpod.dart';

/// The base URL of the gateway the tests talk to. Override with
/// `--dart-define=API_BASE_URL=http://host:port` when the stack runs
/// somewhere other than the default.
const String apiBaseUrl = String.fromEnvironment(
  'API_BASE_URL',
  defaultValue: 'http://localhost:8080',
);

/// Returns the singleton IntegrationTestWidgetsFlutterBinding, ensuring
/// each test file initialises the binding before the first widget pump.
/// Also installs mocks for platform channels whose plugins have no macOS
/// implementation (compass) so their MissingPluginExceptions don't fail
/// every test, and silences known-noisy async errors from MapLibre tile
/// loads (the test env has no MapLibre).
IntegrationTestWidgetsFlutterBinding ensureE2EBinding() {
  final binding = IntegrationTestWidgetsFlutterBinding.ensureInitialized();
  _installPlatformChannelStubs(binding);
  _installErrorFilter();
  return binding;
}

void _installPlatformChannelStubs(IntegrationTestWidgetsFlutterBinding binding) {
  // flutter_compass has no macOS implementation. The app subscribes to its
  // EventChannel on startup; without a stub it throws MissingPluginException
  // on every test, and integration_test treats that as an unexpected error.
  const compass = MethodChannel('hemanthraj/flutter_compass');
  binding.defaultBinaryMessenger.setMockMethodCallHandler(
    compass,
    (call) async {
      // listen / cancel / heading reads → empty / noop.
      return null;
    },
  );
}

void _installErrorFilter() {
  final originalOnError = FlutterError.onError;
  FlutterError.onError = (details) {
    final ex = details.exception;
    // Tile fetches and image-codec resolution against MapLibre fail in
    // headless macOS without a real GL surface; not signal for our tests.
    if (ex.toString().contains('Tile fetch failed') ||
        ex.toString().contains('_TileNotFoundInStore') ||
        // Compass plugin has no macOS impl; we stub the method channel
        // above but EventChannel "listen" still races in some frames.
        ex.toString().contains('hemanthraj/flutter_compass')) {
      return;
    }
    originalOnError?.call(details);
  };
  // Unhandled async errors land in PlatformDispatcher; mirror the filter
  // so MissingPluginException from event-channel teardown doesn't bubble.
  PlatformDispatcher.instance.onError = (error, stack) {
    final msg = error.toString();
    if (msg.contains('hemanthraj/flutter_compass') ||
        msg.contains('Tile fetch failed')) {
      return true; // handled, do not propagate
    }
    return false;
  };
}

/// Generates a unique email per call. Combines the test name (caller
/// supplies a short tag) with a timestamp + random suffix so reruns of
/// the same suite never collide.
String uniqueEmail([String tag = 't']) {
  final ts = DateTime.now().microsecondsSinceEpoch;
  return 'e2e-$tag-$ts@turbo.test';
}

/// Polls the gateway's `/healthz` endpoint until it returns 200 or the
/// timeout elapses. Call once per test file (`setUpAll`) so a flaky
/// stack startup fails loudly rather than as a misleading auth error.
Future<void> waitForBackendHealthy({
  Duration timeout = const Duration(seconds: 30),
}) async {
  final deadline = DateTime.now().add(timeout);
  final dio = Dio(BaseOptions(
    baseUrl: apiBaseUrl,
    connectTimeout: const Duration(seconds: 2),
    receiveTimeout: const Duration(seconds: 2),
    validateStatus: (_) => true,
  ));
  Object? lastError;
  while (DateTime.now().isBefore(deadline)) {
    try {
      final r = await dio.get('/healthz');
      if (r.statusCode == 200) return;
      lastError = 'status ${r.statusCode}';
    } catch (e) {
      lastError = e;
    }
    await Future<void>.delayed(const Duration(milliseconds: 250));
  }
  fail('Backend at $apiBaseUrl did not become healthy within $timeout '
      '(last error: $lastError). Bring the stack up with '
      '`docker compose -f infra/compose/compose.yaml '
      '-f infra/compose/compose.services.yaml up -d`.');
}

/// Result of a direct-HTTP registration. Includes the issued tokens so
/// tests can hit the API directly when they're not driving the login UI.
class RegisteredUser {
  final String email;
  final String password;
  final String accessToken;
  final String refreshToken;
  RegisteredUser({
    required this.email,
    required this.password,
    required this.accessToken,
    required this.refreshToken,
  });
}

/// Registers a fresh user via direct HTTP. Useful when the test wants
/// an authenticated starting state without driving the registration UI.
Future<RegisteredUser> registerNewUser({String tag = 'u'}) async {
  final email = uniqueEmail(tag);
  const password = 'CorrectHorse-Battery-Staple-1!';
  final dio = Dio(BaseOptions(
    baseUrl: apiBaseUrl,
    validateStatus: (_) => true,
  ));
  final r = await dio.post(
    '/api/auth/Auth/register',
    data: {
      'email': email,
      'password': password,
      'confirmPassword': password,
    },
  );
  if (r.statusCode != 200 || r.data is! Map) {
    fail('Direct register failed for $email: ${r.statusCode} ${r.data}');
  }
  final data = r.data as Map<String, dynamic>;
  return RegisteredUser(
    email: email,
    password: password,
    accessToken: data['accessToken'] as String,
    refreshToken: data['refreshToken'] as String,
  );
}

/// Clears SharedPreferences so the cold-start auth gate starts at
/// unauthenticated. Call from `setUp` in every test.
Future<void> resetAppState() async {
  SharedPreferences.setMockInitialValues({});
  final prefs = await SharedPreferences.getInstance();
  await prefs.clear();
}

/// Seeds SharedPreferences with the tokens from a [RegisteredUser] so
/// the next cold start lands in `authenticated`.
Future<void> seedAuthState(RegisteredUser user) async {
  final prefs = await SharedPreferences.getInstance();
  await prefs.setString('accessToken', user.accessToken);
  await prefs.setString('refreshToken', user.refreshToken);
  await prefs.setString('userEmail', user.email);
  await prefs.setBool('isLoggedIn', true);
  await prefs.setBool('isGoogleUser', false);
}

/// Pumps the real `TurboApp` widget, wired with the same activity-kind
/// registry the production `main()` uses, then settles until idle (or
/// the [settleTimeout] expires for screens that have animated splash /
/// network frames).
Future<void> pumpTurboApp(
  WidgetTester tester, {
  Duration settleTimeout = const Duration(seconds: 10),
}) async {
  final activityKinds = activities.ActivityKindRegistry([
    activity_fishing.fishingActivityKindDescriptor,
    activity_backcountry_ski.backcountrySkiActivityKindDescriptor,
    activity_hiking.hikingActivityKindDescriptor,
    activity_xc_ski.xcSkiActivityKindDescriptor,
    activity_packrafting.packraftingActivityKindDescriptor,
    activity_freediving.freedivingActivityKindDescriptor,
  ]);
  await tester.pumpWidget(
    ProviderScope(
      overrides: [
        activities.activityKindRegistryProvider.overrideWithValue(activityKinds),
      ],
      child: const TurboApp(),
    ),
  );
  // Let the cold-start auth gate run (session check or pref read) and
  // the first frame paint.
  await pumpUntilSettled(tester, timeout: settleTimeout);
}

/// Repeatedly pumps until the widget tree settles or [timeout] elapses.
/// `pumpAndSettle` throws on continuous animations (the map's tile
/// loader is one), so this version swallows that and falls back to
/// counted pumps.
Future<void> pumpUntilSettled(
  WidgetTester tester, {
  Duration timeout = const Duration(seconds: 10),
}) async {
  try {
    await tester.pumpAndSettle(const Duration(milliseconds: 100), EnginePhase.sendSemanticsUpdate, timeout);
    return;
  } on Exception {
    // Fall through to counted pumps for screens with perpetual animation.
  }
  final deadline = DateTime.now().add(timeout);
  while (DateTime.now().isBefore(deadline)) {
    await tester.pump(const Duration(milliseconds: 100));
  }
}

/// Polls until [finder] resolves at least one widget or [timeout]
/// elapses. Useful when the UI transitions through async states (auth
/// gate → home → modal).
Future<void> waitFor(
  WidgetTester tester,
  Finder finder, {
  Duration timeout = const Duration(seconds: 10),
}) async {
  final deadline = DateTime.now().add(timeout);
  while (DateTime.now().isBefore(deadline)) {
    await tester.pump(const Duration(milliseconds: 100));
    if (finder.evaluate().isNotEmpty) return;
  }
  fail('Timeout waiting for $finder (waited $timeout). '
      'Visible widgets: ${tester.allWidgets.length}');
}

/// Drives the LoginScreen UI: types email, types password, taps the
/// "Sign in" button (specifically the AppButton — desktop and mobile
/// views both render an extra "Sign in" Text in the header/AppBar).
/// Assumes the LoginScreen is currently visible.
Future<void> submitLoginForm(
  WidgetTester tester, {
  required String email,
  required String password,
}) async {
  await tester.enterText(_emailField(), email);
  await tester.enterText(_passwordField(), password);
  await tester.tap(_appButtonWithText('Sign in'));
  await pumpUntilSettled(tester);
}

/// Drives the RegisterScreen UI: types email, types password, taps the
/// "Create account" AppButton. Assumes the RegisterScreen is visible.
Future<void> submitRegisterForm(
  WidgetTester tester, {
  required String email,
  required String password,
}) async {
  await tester.enterText(_emailField(), email);
  await tester.enterText(_passwordField(), password);
  await tester.tap(_appButtonWithText('Create account'));
  await pumpUntilSettled(tester);
}

Finder _emailField() {
  final f = find.widgetWithText(TextFormField, 'Email');
  expect(f, findsOneWidget, reason: 'Email field not found');
  return f;
}

Finder _passwordField() {
  final f = find.widgetWithText(TextFormField, 'Password');
  expect(f, findsOneWidget, reason: 'Password field not found');
  return f;
}

Finder _appButtonWithText(String text) {
  final f = find.widgetWithText(AppButton, text);
  expect(f, findsOneWidget,
      reason: 'AppButton with text "$text" not found '
          '(${find.text(text).evaluate().length} bare Text matches)');
  return f;
}
