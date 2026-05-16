import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_localizations/flutter_localizations.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:turbo/core/widgets/app_button.dart';
import 'package:turbo/features/auth/api.dart';
import 'package:turbo/l10n/app_localizations.dart';

/// A test double for [AuthStateNotifier] that lets the test drive the outcome
/// of login/register operations.
///
/// It deliberately does NOT instantiate the real [ApiClient] / [AuthService]:
/// overriding [build] keeps us off the network and off platform channels. The
/// fake holds a [Completer] for the in-flight call so the test controls when
/// login resolves and what state it resolves to.
class _FakeAuthNotifier extends AuthStateNotifier {
  int loginCallCount = 0;
  int registerCallCount = 0;
  Completer<void>? _pending;

  @override
  AuthState build() => AuthState(status: AuthStatus.unauthenticated);

  @override
  Future<void> login(String email, String password) async {
    loginCallCount++;
    state = state.copyWith(status: AuthStatus.loading, removeError: true);
    final c = _pending = Completer<void>();
    await c.future;
  }

  @override
  Future<void> register(String email, String password) async {
    registerCallCount++;
    state = state.copyWith(status: AuthStatus.loading, removeError: true);
    final c = _pending = Completer<void>();
    await c.future;
  }

  void resolveSuccess({String email = 'test@example.com'}) {
    state = AuthState(status: AuthStatus.authenticated, email: email);
    _pending?.complete();
    _pending = null;
  }

  void resolveFailure(String errorMessage) {
    state = state.copyWith(
      status: AuthStatus.unauthenticated,
      errorMessage: errorMessage,
    );
    _pending?.complete();
    _pending = null;
  }

  @override
  void clearErrors() {
    state = state.copyWith(removeError: true);
  }
}

/// Hosts an auth screen behind an "open" button so the screen has a Navigator
/// route it can pop on success. The route wraps the pushed screen in a
/// Scaffold so [TextField]s find a Material ancestor regardless of whether
/// the desktop view (no Scaffold) or mobile view (own Scaffold) is selected.
class _AuthHost extends StatelessWidget {
  final Widget screen;
  const _AuthHost({required this.screen});

  @override
  Widget build(BuildContext context) {
    return MaterialApp(
      localizationsDelegates: const [
        AppLocalizations.delegate,
        GlobalMaterialLocalizations.delegate,
        GlobalWidgetsLocalizations.delegate,
        GlobalCupertinoLocalizations.delegate,
      ],
      supportedLocales: AppLocalizations.supportedLocales,
      home: Builder(builder: (ctx) {
        return Scaffold(
          body: Center(
            child: ElevatedButton(
              onPressed: () => Navigator.of(ctx).push(
                MaterialPageRoute<void>(
                  builder: (_) => Scaffold(body: screen),
                ),
              ),
              child: const Text('open'),
            ),
          ),
        );
      }),
    );
  }
}

Future<_FakeAuthNotifier> _openLogin(WidgetTester tester) async {
  final fake = _FakeAuthNotifier();
  await tester.pumpWidget(
    ProviderScope(
      overrides: [authStateProvider.overrideWith(() => fake)],
      child: const _AuthHost(screen: LoginScreen()),
    ),
  );
  await tester.pumpAndSettle();
  await tester.tap(find.text('open'));
  await tester.pumpAndSettle();
  return fake;
}

Future<_FakeAuthNotifier> _openRegister(WidgetTester tester) async {
  final fake = _FakeAuthNotifier();
  await tester.pumpWidget(
    ProviderScope(
      overrides: [authStateProvider.overrideWith(() => fake)],
      child: const _AuthHost(screen: RegisterScreen()),
    ),
  );
  await tester.pumpAndSettle();
  await tester.tap(find.text('open'));
  await tester.pumpAndSettle();
  return fake;
}

Future<void> _enterCredentials(
  WidgetTester tester, {
  String email = 'user@example.com',
  String password = 'longenoughpassword',
}) async {
  final emailField = find.widgetWithText(TextFormField, 'Email');
  final passwordField = find.widgetWithText(TextFormField, 'Password');
  expect(emailField, findsOneWidget);
  expect(passwordField, findsOneWidget);
  await tester.enterText(emailField, email);
  await tester.enterText(passwordField, password);
  await tester.pump();
}

AppButton _primaryButton(WidgetTester tester) =>
    tester.widget<AppButton>(find.byType(AppButton));

void main() {
  group('LoginScreen user-facing flow', () {
    testWidgets('form validation blocks submission when fields are empty',
        (tester) async {
      final fake = await _openLogin(tester);

      await tester.tap(find.byType(AppButton));
      await tester.pumpAndSettle();

      // The user sees validator errors.
      expect(find.text('Please enter your email'), findsOneWidget);
      expect(find.text('Please enter your password'), findsOneWidget);
      // No login was initiated.
      expect(fake.loginCallCount, 0);
      // Screen is still here.
      expect(find.byType(LoginScreen), findsOneWidget);
    });

    testWidgets('tapping Sign In with valid credentials invokes login once '
        'and disables the button', (tester) async {
      final fake = await _openLogin(tester);
      await _enterCredentials(tester);

      await tester.tap(find.byType(AppButton));
      await tester.pump(); // process the tap + setState

      expect(fake.loginCallCount, 1);
      // While the request is in flight the primary action is disabled — the
      // user can't double-submit by tapping again.
      expect(_primaryButton(tester).isLoading, isTrue);

      await tester.tap(find.byType(AppButton), warnIfMissed: false);
      await tester.pump();
      expect(fake.loginCallCount, 1,
          reason: 'second tap should be ignored while login is in flight');
    });

    testWidgets('successful login pops the screen', (tester) async {
      final fake = await _openLogin(tester);
      await _enterCredentials(tester);

      await tester.tap(find.byType(AppButton));
      await tester.pump();
      expect(find.byType(LoginScreen), findsOneWidget);

      fake.resolveSuccess();
      await tester.pumpAndSettle();

      // The user is back on the host screen — the login screen closed itself.
      expect(find.byType(LoginScreen), findsNothing);
      expect(find.text('open'), findsOneWidget);
    });

    testWidgets('failed login shows error, re-enables button, and keeps the '
        'screen open', (tester) async {
      final fake = await _openLogin(tester);
      await _enterCredentials(tester);

      await tester.tap(find.byType(AppButton));
      await tester.pump();

      fake.resolveFailure('Exception: Bad credentials');
      await tester.pumpAndSettle();

      // Error message visible to the user.
      expect(find.textContaining('Bad credentials'), findsOneWidget);
      // Button is interactive again — user can retry.
      expect(_primaryButton(tester).isLoading, isFalse);
      // Screen did not close.
      expect(find.byType(LoginScreen), findsOneWidget);
    });

    testWidgets('user can retry login after a failure', (tester) async {
      final fake = await _openLogin(tester);
      await _enterCredentials(tester);

      await tester.tap(find.byType(AppButton));
      await tester.pump();
      fake.resolveFailure('Exception: Bad credentials');
      await tester.pumpAndSettle();

      // Second attempt — same credentials, this time it succeeds.
      await tester.tap(find.byType(AppButton));
      await tester.pump();
      expect(fake.loginCallCount, 2);

      fake.resolveSuccess();
      await tester.pumpAndSettle();
      expect(find.byType(LoginScreen), findsNothing);
    });

    testWidgets('typing in a field clears a previously shown error',
        (tester) async {
      final fake = await _openLogin(tester);
      await _enterCredentials(tester);
      await tester.tap(find.byType(AppButton));
      await tester.pump();
      fake.resolveFailure('Exception: Bad credentials');
      await tester.pumpAndSettle();

      expect(find.textContaining('Bad credentials'), findsOneWidget);

      // The user starts typing — the error banner clears.
      await tester.enterText(
        find.widgetWithText(TextFormField, 'Email'),
        'someone@else.com',
      );
      await tester.pumpAndSettle();

      expect(find.textContaining('Bad credentials'), findsNothing);
    });
  });

  group('RegisterScreen user-facing flow', () {
    testWidgets('form validation rejects malformed email', (tester) async {
      final fake = await _openRegister(tester);

      await tester.enterText(
        find.widgetWithText(TextFormField, 'Email'),
        'not-an-email',
      );
      await tester.enterText(
        find.widgetWithText(TextFormField, 'Password'),
        'longenoughpassword',
      );
      await tester.tap(find.byType(AppButton));
      await tester.pumpAndSettle();

      expect(find.text('Please enter a valid email'), findsOneWidget);
      expect(fake.registerCallCount, 0);
    });

    testWidgets('form validation rejects short passwords', (tester) async {
      final fake = await _openRegister(tester);

      await tester.enterText(
        find.widgetWithText(TextFormField, 'Email'),
        'a@a.aa',
      );
      await tester.enterText(
        find.widgetWithText(TextFormField, 'Password'),
        'short',
      );
      await tester.tap(find.byType(AppButton));
      await tester.pumpAndSettle();

      expect(find.textContaining('Password must be at least'), findsOneWidget);
      expect(fake.registerCallCount, 0);
    });

    testWidgets('successful registration pops the screen', (tester) async {
      final fake = await _openRegister(tester);
      await tester.enterText(
        find.widgetWithText(TextFormField, 'Email'),
        'newuser@example.com',
      );
      await tester.enterText(
        find.widgetWithText(TextFormField, 'Password'),
        'longenoughpassword',
      );

      await tester.tap(find.byType(AppButton));
      await tester.pump();
      expect(fake.registerCallCount, 1);
      expect(_primaryButton(tester).isLoading, isTrue);

      fake.resolveSuccess(email: 'newuser@example.com');
      await tester.pumpAndSettle();

      expect(find.byType(RegisterScreen), findsNothing);
      expect(find.text('open'), findsOneWidget);
    });

    testWidgets('failed registration keeps the screen and shows the error',
        (tester) async {
      final fake = await _openRegister(tester);
      await tester.enterText(
        find.widgetWithText(TextFormField, 'Email'),
        'taken@example.com',
      );
      await tester.enterText(
        find.widgetWithText(TextFormField, 'Password'),
        'longenoughpassword',
      );

      await tester.tap(find.byType(AppButton));
      await tester.pump();
      fake.resolveFailure('Exception: Email already registered');
      await tester.pumpAndSettle();

      expect(find.textContaining('Email already registered'), findsOneWidget);
      expect(_primaryButton(tester).isLoading, isFalse);
      expect(find.byType(RegisterScreen), findsOneWidget);
    });
  });
}
