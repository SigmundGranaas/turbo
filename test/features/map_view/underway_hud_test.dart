import 'package:flutter/material.dart';
import 'package:flutter_localizations/flutter_localizations.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:latlong2/latlong.dart';
import 'package:shared_preferences/shared_preferences.dart';
import 'package:turbo/core/location/compass_state.dart';
import 'package:turbo/core/location/location_state.dart';
import 'package:turbo/features/map_view/widgets/underway_hud.dart';
import 'package:turbo/features/settings/api.dart';
import 'package:turbo/app/l10n/app_localizations.dart';

class _SeededPosition extends LastPositionNotifier {
  final PositionSnapshot? seed;
  _SeededPosition(this.seed);
  @override
  PositionSnapshot? build() => seed;
}

Future<void> _pumpHud(
  WidgetTester tester, {
  required bool hudEnabled,
  DistanceUnit unit = DistanceUnit.nautical,
  PositionSnapshot? position,
  double? compassHeading,
}) async {
  SharedPreferences.setMockInitialValues({
    'showUnderwayHud': hudEnabled,
    'distanceUnit': unit.name,
  });
  await tester.pumpWidget(
    ProviderScope(
      overrides: [
        lastPositionProvider.overrideWith(() => _SeededPosition(position)),
        compassStateProvider
            .overrideWith((ref) => Stream.value(compassHeading)),
      ],
      child: const MaterialApp(
        localizationsDelegates: [
          AppLocalizations.delegate,
          GlobalMaterialLocalizations.delegate,
          GlobalWidgetsLocalizations.delegate,
          GlobalCupertinoLocalizations.delegate,
        ],
        supportedLocales: AppLocalizations.supportedLocales,
        home: Scaffold(body: UnderwayHud()),
      ),
    ),
  );
  await tester.pumpAndSettle();
}

void main() {
  group('UnderwayHud', () {
    testWidgets('renders nothing when the setting is off', (tester) async {
      await _pumpHud(
        tester,
        hudEnabled: false,
        position: PositionSnapshot(
          latLng: const LatLng(60, 5),
          speedMps: 5,
          courseDeg: 90,
        ),
      );
      expect(find.text('SOG'), findsNothing);
      expect(find.text('COG'), findsNothing);
      expect(find.text('HDG'), findsNothing);
    });

    testWidgets('shows SOG/COG/HDG labels and zero-padded bearings in nautical mode',
        (tester) async {
      await _pumpHud(
        tester,
        hudEnabled: true,
        unit: DistanceUnit.nautical,
        position: PositionSnapshot(
          latLng: const LatLng(60, 5),
          // 5 m/s ≈ 9.7 kn
          speedMps: 5,
          courseDeg: 12,
        ),
        compassHeading: 184,
      );

      expect(find.text('SOG'), findsOneWidget);
      expect(find.text('COG'), findsOneWidget);
      expect(find.text('HDG'), findsOneWidget);

      // Speed in knots
      expect(find.text('9.7 kn'), findsOneWidget);
      // Bearings should be zero-padded to three digits.
      expect(find.text('012°'), findsOneWidget);
      expect(find.text('184°'), findsOneWidget);
    });

    testWidgets('falls back to em-dash when GPS values are unavailable',
        (tester) async {
      await _pumpHud(
        tester,
        hudEnabled: true,
        unit: DistanceUnit.nautical,
        position: null,
        compassHeading: null,
      );
      // Three em-dashes — one per stat field.
      expect(find.text('—'), findsNWidgets(3));
    });

    testWidgets('uses km/h when the unit setting is metric', (tester) async {
      await _pumpHud(
        tester,
        hudEnabled: true,
        unit: DistanceUnit.metric,
        position: PositionSnapshot(
          latLng: const LatLng(60, 5),
          speedMps: 10,
          courseDeg: 0,
        ),
        compassHeading: 0,
      );
      // 10 m/s = 36.0 km/h
      expect(find.text('36.0 km/h'), findsOneWidget);
      expect(find.text('000°'), findsNWidgets(2));
    });
  });
}
