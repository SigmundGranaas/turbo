import 'package:flutter_test/flutter_test.dart';
import 'package:turbo/features/weather/api.dart';

void main() {
  group('WeatherSymbol', () {
    test('known symbol code maps to the matching asset path', () {
      final s = WeatherSymbol.fromCode('clearsky_day');
      expect(s.code, 'clearsky_day');
      expect(s.assetPath, 'assets/weather/clearsky_day.svg');
      expect(s.isFallback, isFalse);
    });

    test('unknown code falls back to a generic symbol', () {
      final s = WeatherSymbol.fromCode('definitely_not_a_real_code');
      expect(s.isFallback, isTrue);
      expect(s.assetPath, 'assets/weather/cloudy.svg');
    });

    test('null or empty code falls back', () {
      expect(WeatherSymbol.fromCode(null).isFallback, isTrue);
      expect(WeatherSymbol.fromCode('').isFallback, isTrue);
    });

    test('isSnow recognizes pure snow codes', () {
      expect(WeatherSymbol.fromCode('snow').isSnow, isTrue);
      expect(WeatherSymbol.fromCode('lightsnow').isSnow, isTrue);
      expect(WeatherSymbol.fromCode('heavysnowshowers_day').isSnow, isTrue);
      expect(WeatherSymbol.fromCode('snowshowersandthunder_night').isSnow,
          isTrue);
    });

    test('isSnow recognizes sleet codes', () {
      expect(WeatherSymbol.fromCode('sleet').isSnow, isTrue);
      expect(WeatherSymbol.fromCode('lightsleetshowers_polartwilight').isSnow,
          isTrue);
    });

    test('isSnow is false for rain / clearsky / unknown', () {
      expect(WeatherSymbol.fromCode('rain').isSnow, isFalse);
      expect(WeatherSymbol.fromCode('clearsky_day').isSnow, isFalse);
      expect(WeatherSymbol.fromCode('lightrainshowers_day').isSnow, isFalse);
      expect(WeatherSymbol.fromCode(null).isSnow, isFalse);
    });
  });
}
