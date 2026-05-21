/// User-selectable distance unit. Persisted by the settings feature.
enum DistanceUnit {
  metric,
  imperial;

  static DistanceUnit fromName(String? name) {
    return switch (name) {
      'imperial' => DistanceUnit.imperial,
      _ => DistanceUnit.metric,
    };
  }
}

const double _metersPerMile = 1609.344;
const double _metersPerFoot = 0.3048;

/// Renders a distance in meters using the selected unit, switching between
/// the small unit (m / ft) and the large unit (km / mi) at a sensible
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
  }
}
