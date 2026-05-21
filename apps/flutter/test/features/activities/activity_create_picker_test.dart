import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:latlong2/latlong.dart';
import 'package:turbo/features/activities/api.dart' as activities;
import 'package:turbo/features/auth/api.dart';

import '../../helpers/pump_app.dart';

/// Widget tests for the picker's auth-state branching.
///
/// Activities are owner-scoped on the server — every endpoint requires a
/// JWT. The picker is the choke point shared by every entry point that can
/// open the kind chooser (long-press, "save as activity" on a marker or
/// saved path). It must render a sign-in CTA for anonymous users so we
/// don't let them fill out a kind-specific form that's guaranteed to 401
/// on submit.

class _AuthNotifier extends AuthStateNotifier {
  final AuthStatus _initial;
  _AuthNotifier(this._initial);

  @override
  AuthState build() => AuthState(status: _initial, email: 'maybe@user.com');

  void signIn() => state = AuthState(status: AuthStatus.authenticated, email: 'in@user.com');
  void signOut() => state = AuthState(status: AuthStatus.unauthenticated);
}

void main() {
  final seedPoint = LatLng(60.0, 10.0);
  final seedPointGeometry = activities.ActivityGeometry.fromPoint(seedPoint);

  testWidgets('anonymous user sees a sign-in CTA instead of the kind list',
      (tester) async {
    final auth = _AuthNotifier(AuthStatus.unauthenticated);

    await pumpTestApp(
      tester,
      activities.ActivityCreatePicker.fromPoint(seedPoint),
      overrides: [authStateProvider.overrideWith(() => auth)],
    );

    expect(find.text('Sign in to add activities'), findsOneWidget);
    expect(find.text('Sign in'), findsOneWidget);
    expect(find.text('Not now'), findsOneWidget);
    // No kind tiles should appear behind the CTA.
    expect(find.text('New activity'), findsNothing);
  });

  testWidgets('authenticated user sees the New-activity headline + kind tiles',
      (tester) async {
    final auth = _AuthNotifier(AuthStatus.authenticated);

    await pumpTestApp(
      tester,
      activities.ActivityCreatePicker(seedGeometry: seedPointGeometry),
      overrides: [authStateProvider.overrideWith(() => auth)],
    );

    expect(find.text('New activity'), findsOneWidget);
    expect(find.textContaining('Pick a kind'), findsOneWidget);
    expect(find.text('Sign in to add activities'), findsNothing);
  });

  testWidgets('picker rebuilds from sign-in CTA to kind list when auth changes',
      (tester) async {
    final auth = _AuthNotifier(AuthStatus.unauthenticated);

    await pumpTestApp(
      tester,
      activities.ActivityCreatePicker.fromPoint(seedPoint),
      overrides: [authStateProvider.overrideWith(() => auth)],
    );

    expect(find.text('Sign in to add activities'), findsOneWidget);

    auth.signIn();
    await tester.pumpAndSettle();

    expect(find.text('Sign in to add activities'), findsNothing);
    expect(find.text('New activity'), findsOneWidget,
        reason: 'Picker must react to a state change to authenticated.');
  });

  testWidgets(
      'tapping "Not now" pops the picker without navigating to sign-in',
      (tester) async {
    final auth = _AuthNotifier(AuthStatus.unauthenticated);

    // Wrap in a Navigator so we can pop.
    await pumpTestApp(
      tester,
      Builder(
        builder: (ctx) => ElevatedButton(
          onPressed: () => showModalBottomSheet<void>(
            context: ctx,
            builder: (_) => activities.ActivityCreatePicker.fromPoint(seedPoint),
          ),
          child: const Text('open'),
        ),
      ),
      overrides: [authStateProvider.overrideWith(() => auth)],
    );

    await tester.tap(find.text('open'));
    await tester.pumpAndSettle();
    expect(find.text('Sign in to add activities'), findsOneWidget);

    await tester.tap(find.text('Not now'));
    await tester.pumpAndSettle();
    expect(find.text('Sign in to add activities'), findsNothing,
        reason: 'Not-now must dismiss the picker without further side effects.');
  });
}
