import 'dart:math' as math;

/// One day's sun timeline at a fixed coordinate, fetched from MET Norway's
/// Sunrise 3 API.
class SunEvent {
  /// Local date (midnight in the local time zone the request was issued for).
  final DateTime date;
  final DateTime? sunrise;
  final DateTime? sunset;
  final DateTime? solarNoon;
  final DateTime? solarMidnight;

  /// True when the sun never sets on [date] (polar day).
  final bool polarDay;

  /// True when the sun never rises on [date] (polar night).
  final bool polarNight;

  const SunEvent({
    required this.date,
    required this.sunrise,
    required this.sunset,
    required this.solarNoon,
    required this.solarMidnight,
    required this.polarDay,
    required this.polarNight,
  });

  Duration? get daylight {
    final r = sunrise;
    final s = sunset;
    if (polarDay) return const Duration(hours: 24);
    if (polarNight) return Duration.zero;
    if (r == null || s == null) return null;
    if (!s.isAfter(r)) return null;
    return s.difference(r);
  }
}

/// Moonrise / moonset and phase for a given local date.
class MoonEvent {
  final DateTime date;
  final DateTime? moonrise;
  final DateTime? moonset;

  /// Moon phase as a number in `[0, 360)` degrees where 0 = new moon,
  /// 90 = first quarter, 180 = full moon, 270 = last quarter.
  final double? phaseDegrees;

  /// Fraction of the moon's disc illuminated, `[0, 1]`.
  double? get illumination {
    final p = phaseDegrees;
    if (p == null) return null;
    final rad = (p % 360) / 360 * 2 * math.pi;
    return (1 - math.cos(rad)) / 2;
  }

  const MoonEvent({
    required this.date,
    required this.moonrise,
    required this.moonset,
    required this.phaseDegrees,
  });
}
