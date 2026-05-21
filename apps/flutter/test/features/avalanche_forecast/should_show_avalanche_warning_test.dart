import 'package:flutter_test/flutter_test.dart';
import 'package:turbo/features/avalanche_forecast/api.dart';

AvalancheWarning _warning(AvalancheDangerLevel level) => AvalancheWarning(
      regionId: 3018,
      regionName: 'Salten',
      validDate: DateTime(2026, 5, 18),
      dangerLevel: level,
      mainText: null,
      avalancheDanger: null,
      problems: const [],
    );

void main() {
  group('shouldShowAvalancheWarning', () {
    test('hides level 1 (Low) regardless of temperature', () {
      final w = _warning(AvalancheDangerLevel.low);
      expect(shouldShowAvalancheWarning(w, currentAirTempC: -10), isFalse);
      expect(shouldShowAvalancheWarning(w, currentAirTempC: 0), isFalse);
      expect(shouldShowAvalancheWarning(w, currentAirTempC: 15), isFalse);
      expect(shouldShowAvalancheWarning(w, currentAirTempC: null), isFalse);
    });

    test('hides level 2 (Moderate) when temperature is well above freezing',
        () {
      final w = _warning(AvalancheDangerLevel.moderate);
      expect(shouldShowAvalancheWarning(w, currentAirTempC: 10), isFalse);
      expect(shouldShowAvalancheWarning(w, currentAirTempC: 6), isFalse);
    });

    test('shows level 2 (Moderate) when temperature is cool or unknown', () {
      final w = _warning(AvalancheDangerLevel.moderate);
      expect(shouldShowAvalancheWarning(w, currentAirTempC: 0), isTrue);
      expect(shouldShowAvalancheWarning(w, currentAirTempC: -5), isTrue);
      expect(shouldShowAvalancheWarning(w, currentAirTempC: 5), isTrue);
      expect(shouldShowAvalancheWarning(w, currentAirTempC: null), isTrue,
          reason:
              'Missing temperature defaults to showing so a brief weather-fetch '
              'lag never silently suppresses a real warning.');
    });

    test('always shows level 3 (Considerable) and above', () {
      for (final level in [
        AvalancheDangerLevel.considerable,
        AvalancheDangerLevel.high,
        AvalancheDangerLevel.extreme,
      ]) {
        final w = _warning(level);
        expect(shouldShowAvalancheWarning(w, currentAirTempC: 20), isTrue,
            reason: '$level should show even at 20°C');
        expect(shouldShowAvalancheWarning(w, currentAirTempC: null), isTrue,
            reason: '$level should show even when temperature is unknown');
      }
    });
  });
}
