/// User-selectable distance unit. Persisted by the settings feature.
enum DistanceUnit {
  metric,
  imperial,
  nautical;

  static DistanceUnit fromName(String? name) {
    return switch (name) {
      'imperial' => DistanceUnit.imperial,
      'nautical' => DistanceUnit.nautical,
      _ => DistanceUnit.metric,
    };
  }
}

const double _metersPerMile = 1609.344;
const double _metersPerFoot = 0.3048;
const double _metersPerNauticalMile = 1852.0;

/// Renders a distance in meters using the selected unit, switching between
/// the small unit (m / ft) and the large unit (km / mi / NM) at a sensible
/// threshold. Always returns a non-localized number; the calling widget is
/// responsible for any l10n the surrounding sentence needs.
String formatDistance(double meters, DistanceUnit unit) {
  if (meters.isNaN || meters.isInfinite) return '—';
  switch (unit) {
    case DistanceUnit.metric:
      if (meters < 1000) return '${meters.round()} m';
      return '${(meters / 1000).toStringAsFixed(2)} km';
    case DistanceUnit.imperial:
      final feet = meters / _metersPerFoot;
      if (feet < 1000) return '${feet.round()} ft';
      final miles = meters / _metersPerMile;
      return '${miles.toStringAsFixed(2)} mi';
    case DistanceUnit.nautical:
      // Below 1 NM, fall back to meters so close-quarters distances stay
      // readable (chart plotters do the same with cables/meters).
      if (meters < _metersPerNauticalMile) return '${meters.round()} m';
      return '${(meters / _metersPerNauticalMile).toStringAsFixed(2)} NM';
  }
}

/// Renders a speed in m/s using the selected unit:
/// km/h (metric), mph (imperial), or knots (nautical).
String formatSpeed(double metersPerSecond, DistanceUnit unit) {
  if (metersPerSecond.isNaN || metersPerSecond.isInfinite) return '—';
  // Negative speeds shouldn't occur from GPS but guard anyway.
  final v = metersPerSecond < 0 ? 0.0 : metersPerSecond;
  switch (unit) {
    case DistanceUnit.metric:
      return '${(v * 3.6).toStringAsFixed(1)} km/h';
    case DistanceUnit.imperial:
      return '${(v * 2.2369362921).toStringAsFixed(1)} mph';
    case DistanceUnit.nautical:
      return '${(v * 1.9438444924).toStringAsFixed(1)} kn';
  }
}
