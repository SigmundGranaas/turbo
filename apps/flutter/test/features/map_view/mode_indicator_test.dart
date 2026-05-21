import 'package:flutter/material.dart';
import 'package:flutter_localizations/flutter_localizations.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:latlong2/latlong.dart';
import 'package:turbo/core/location/compass_mode_state.dart';
import 'package:turbo/core/location/compass_state.dart';
import 'package:turbo/core/location/follow_mode_state.dart';
import 'package:turbo/core/location/location_state.dart';
import 'package:turbo/features/map_view/widgets/mode_indicator.dart';
import 'package:turbo/features/navigation/api.dart';
import 'package:turbo/app/l10n/app_localizations.dart';

class _FakeLocation extends LocationState {
  final LatLng? value;
  _FakeLocation(this.value);
  @override
  Future<LatLng?> build() async => value;
}

class _FakeNav extends NavigationStateNotifier {
  final NavigationState initial;
  int stopCallCount = 0;
  _FakeNav(this.initial);
  @override
  NavigationState build() => initial;
  @override
  void stopNavigation() {
    stopCallCount++;
    state = NavigationState.inactive;
  }
}

Future<_FakeNav> _pump(
  WidgetTester tester, {
  bool follow = false,
  bool compass = false,
  NavigationState nav = NavigationState.inactive,
  LatLng? location,
  double? heading,
}) async {
  final navNotifier = _FakeNav(nav);
  final initial = follow ? FollowMode.active : FollowMode.off;
  await tester.pumpWidget(
    ProviderScope(
      overrides: [
        followModeProvider.overrideWith(() => _SeededFollow(initial)),
        compassModeProvider.overrideWith(() => _SeededCompass(compass)),
        navigationStateProvider.overrideWith(() => navNotifier),
        locationStateProvider.overrideWith(() => _FakeLocation(location)),
        compassStateProvider
            .overrideWith((ref) => Stream.value(heading)),
      ],
      child: const MaterialApp(
        localizationsDelegates: [
          AppLocalizations.delegate,
          GlobalMaterialLocalizations.delegate,
          GlobalWidgetsLocalizations.delegate,
          GlobalCupertinoLocalizations.delegate,
        ],
        supportedLocales: AppLocalizations.supportedLocales,
        home: Scaffold(body: Stack(children: [ModeIndicator()])),
      ),
    ),
  );
  await tester.pumpAndSettle();
  return navNotifier;
}

class _SeededFollow extends FollowModeNotifier {
  final FollowMode _seed;
  _SeededFollow(this._seed);
  @override
  FollowMode build() => _seed;
}

class _SeededCompass extends CompassModeNotifier {
  final bool _seed;
  _SeededCompass(this._seed);
  @override
  bool build() => _seed;
}

void main() {
  group('ModeIndicator visibility', () {
    testWidgets('renders nothing when no mode is active', (tester) async {
      await _pump(tester);
      expect(find.byIcon(Icons.my_location), findsNothing);
      expect(find.byIcon(Icons.explore), findsNothing);
    });

    testWidgets('shows the follow chip when follow mode is on',
        (tester) async {
      await _pump(tester, follow: true);
      expect(find.byIcon(Icons.my_location), findsOneWidget);
      expect(find.text('Following'), findsOneWidget);
    });

    testWidgets('shows the compass chip when compass mode is on',
        (tester) async {
      await _pump(tester, compass: true, heading: 45);
      expect(find.byIcon(Icons.explore), findsOneWidget);
      // Cardinal label format: "NE 45°"
      expect(find.textContaining('NE'), findsOneWidget);
      expect(find.textContaining('45'), findsOneWidget);
    });

    testWidgets('shows the navigation chip when navigation is active',
        (tester) async {
      await _pump(
        tester,
        nav: NavigationState(target: const LatLng(63.5, 10.5), isActive: true),
        location: const LatLng(63.4, 10.4),
      );
      expect(find.byIcon(Icons.navigation), findsOneWidget);
    });
  });

  group('ModeIndicator touch targets (regression: was bare Icon size 20)',
      () {
    testWidgets('navigation chip close button uses IconButton (48dp target)',
        (tester) async {
      await _pump(
        tester,
        nav: NavigationState(target: const LatLng(63.5, 10.5), isActive: true),
        location: const LatLng(63.4, 10.4),
      );
      // The close icon inside the chip is now wrapped in IconButton.
      final closeFinder = find.descendant(
        of: find.byType(IconButton),
        matching: find.byIcon(Icons.close),
      );
      expect(closeFinder, findsWidgets,
          reason: 'close icon must be inside an IconButton for the 48dp target');
    });

    testWidgets('tapping the navigation close button calls stopNavigation',
        (tester) async {
      final nav = await _pump(
        tester,
        nav: NavigationState(target: const LatLng(63.5, 10.5), isActive: true),
        location: const LatLng(63.4, 10.4),
      );
      await tester.tap(find.descendant(
        of: find.byType(IconButton),
        matching: find.byIcon(Icons.close),
      ));
      await tester.pumpAndSettle();
      expect(nav.stopCallCount, 1);
    });

    testWidgets('follow chip close button uses IconButton', (tester) async {
      await _pump(tester, follow: true);
      final closeFinder = find.descendant(
        of: find.byType(IconButton),
        matching: find.byIcon(Icons.close),
      );
      expect(closeFinder, findsOneWidget);
    });
  });
}
