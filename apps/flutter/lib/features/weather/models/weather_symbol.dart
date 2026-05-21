/// Wrapper around a MET Norway weather symbol code.
///
/// MET ships 83 distinct codes (e.g. `clearsky_day`, `lightrainshowers_night`,
/// `heavysnowshowersandthunder_polartwilight`). Each one has a matching SVG
/// asset under `assets/weather/`, bundled from metno/weathericons (MIT).
/// Unknown codes (e.g. a future addition) fall back to a generic cloudy icon
/// so the UI never crashes.
class WeatherSymbol {
  /// All canonical symbol codes shipped by MET Norway.
  ///
  /// Source: https://github.com/metno/weathericons (MIT license — bundled at
  /// `assets/weather/LICENSE`). Encoded here to make membership tests cheap
  /// and to avoid filesystem I/O.
  static const Set<String> _knownCodes = {
    'clearsky_day',
    'clearsky_night',
    'clearsky_polartwilight',
    'fair_day',
    'fair_night',
    'fair_polartwilight',
    'partlycloudy_day',
    'partlycloudy_night',
    'partlycloudy_polartwilight',
    'cloudy',
    'fog',
    'rain',
    'rainshowers_day',
    'rainshowers_night',
    'rainshowers_polartwilight',
    'lightrain',
    'lightrainshowers_day',
    'lightrainshowers_night',
    'lightrainshowers_polartwilight',
    'heavyrain',
    'heavyrainshowers_day',
    'heavyrainshowers_night',
    'heavyrainshowers_polartwilight',
    'rainandthunder',
    'rainshowersandthunder_day',
    'rainshowersandthunder_night',
    'rainshowersandthunder_polartwilight',
    'lightrainandthunder',
    'lightrainshowersandthunder_day',
    'lightrainshowersandthunder_night',
    'lightrainshowersandthunder_polartwilight',
    'heavyrainandthunder',
    'heavyrainshowersandthunder_day',
    'heavyrainshowersandthunder_night',
    'heavyrainshowersandthunder_polartwilight',
    'sleet',
    'sleetshowers_day',
    'sleetshowers_night',
    'sleetshowers_polartwilight',
    'lightsleet',
    'lightsleetshowers_day',
    'lightsleetshowers_night',
    'lightsleetshowers_polartwilight',
    'heavysleet',
    'heavysleetshowers_day',
    'heavysleetshowers_night',
    'heavysleetshowers_polartwilight',
    'sleetandthunder',
    'sleetshowersandthunder_day',
    'sleetshowersandthunder_night',
    'sleetshowersandthunder_polartwilight',
    'lightsleetandthunder',
    'lightssleetshowersandthunder_day',
    'lightssleetshowersandthunder_night',
    'lightssleetshowersandthunder_polartwilight',
    'heavysleetandthunder',
    'heavysleetshowersandthunder_day',
    'heavysleetshowersandthunder_night',
    'heavysleetshowersandthunder_polartwilight',
    'snow',
    'snowshowers_day',
    'snowshowers_night',
    'snowshowers_polartwilight',
    'lightsnow',
    'lightsnowshowers_day',
    'lightsnowshowers_night',
    'lightsnowshowers_polartwilight',
    'heavysnow',
    'heavysnowshowers_day',
    'heavysnowshowers_night',
    'heavysnowshowers_polartwilight',
    'snowandthunder',
    'snowshowersandthunder_day',
    'snowshowersandthunder_night',
    'snowshowersandthunder_polartwilight',
    'lightsnowandthunder',
    'lightssnowshowersandthunder_day',
    'lightssnowshowersandthunder_night',
    'lightssnowshowersandthunder_polartwilight',
    'heavysnowandthunder',
    'heavysnowshowersandthunder_day',
    'heavysnowshowersandthunder_night',
    'heavysnowshowersandthunder_polartwilight',
  };

  static const String _fallbackCode = 'cloudy';

  final String code;
  final bool isFallback;

  const WeatherSymbol._(this.code, {this.isFallback = false});

  factory WeatherSymbol.fromCode(String? code) {
    if (code == null || code.isEmpty) {
      return const WeatherSymbol._(_fallbackCode, isFallback: true);
    }
    if (_knownCodes.contains(code)) {
      return WeatherSymbol._(code);
    }
    return const WeatherSymbol._(_fallbackCode, isFallback: true);
  }

  String get assetPath => 'assets/weather/$code.svg';

  /// True when the symbol indicates snow or sleet precipitation. Used to drive
  /// the user-facing "snow" metric without requiring a separate API call.
  bool get isSnow {
    if (isFallback) return false;
    return code.contains('snow') || code.contains('sleet');
  }

  @override
  bool operator ==(Object other) =>
      identical(this, other) ||
      (other is WeatherSymbol &&
          other.code == code &&
          other.isFallback == isFallback);

  @override
  int get hashCode => Object.hash(code, isFallback);

  @override
  String toString() => 'WeatherSymbol($code${isFallback ? ', fallback' : ''})';
}
