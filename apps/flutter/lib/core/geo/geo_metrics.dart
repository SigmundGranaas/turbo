import 'dart:math' as math;

import 'package:latlong2/latlong.dart';

/// How far along a path a position is, and how far off it.
///
/// Produced by [GeoMetrics.progress]; consumed by the active-journey feature to
/// render live "X km to go / on route" without each feature re-deriving it.
class PathProgress {
  /// Index `i` of the closest segment `points[i] → points[i+1]`.
  final int segmentIndex;

  /// The closest point on the path to the queried position.
  final LatLng snapped;

  /// Distance from the path start to [snapped], metres.
  final double distanceAlongM;

  /// Remaining distance from [snapped] to the path end, metres.
  final double remainingM;

  /// Perpendicular distance from the queried position to [snapped], metres.
  /// A large value means the user has wandered off the route.
  final double offRouteM;

  /// [distanceAlongM] / total length, clamped to 0‥1.
  final double fraction;

  const PathProgress({
    required this.segmentIndex,
    required this.snapped,
    required this.distanceAlongM,
    required this.remainingM,
    required this.offRouteM,
    required this.fraction,
  });
}

/// The single source of truth for path geometry maths: length, bearing, ETA,
/// and progress-along-path. Replaces the three hand-rolled distance loops that
/// lived in navigation, recording, and route display. Pure functions, no state.
class GeoMetrics {
  GeoMetrics._();

  static const Distance _distance = Distance();

  /// Approximate metres-per-degree longitude at [latDeg] (equirectangular).
  static const double _metresPerDegLat = 110540.0;
  static double _metresPerDegLng(double latDeg) =>
      111320.0 * math.cos(latDeg * math.pi / 180.0);

  /// Geodesic distance between two points, metres.
  static double distanceMeters(LatLng a, LatLng b) => _distance.distance(a, b);

  /// Total polyline length, metres. 0 for fewer than two points.
  static double pathLengthMeters(List<LatLng> points) {
    if (points.length < 2) return 0;
    double total = 0;
    for (var i = 0; i < points.length - 1; i++) {
      total += _distance.distance(points[i], points[i + 1]);
    }
    return total;
  }

  /// Initial bearing from [from] to [to], degrees clockwise from north (0‥360).
  static double bearingDegrees(LatLng from, LatLng to) {
    final bearing = _distance.bearing(from, to);
    return (bearing % 360 + 360) % 360;
  }

  /// Naismith-style ETA: flat pace plus an ascent penalty.
  ///
  /// Defaults: 5 km/h on the flat, +10 min per 100 m climbed — the common
  /// walking rule of thumb. Returns seconds.
  static double naismithSeconds(
    double distanceM, {
    double ascentM = 0,
    double flatSpeedMps = 5000 / 3600,
    double ascentSecondsPerMeter = 600 / 100,
  }) {
    final flat = distanceM / flatSpeedMps;
    final climb = ascentM > 0 ? ascentM * ascentSecondsPerMeter : 0;
    return flat + climb;
  }

  /// Sum of positive elevation deltas (ascent) and negative deltas (descent)
  /// across the sequence, ignoring nulls. Returns `(ascent, descent)` in metres.
  static ({double ascent, double descent}) ascentDescent(
      List<double?> elevations) {
    double ascent = 0;
    double descent = 0;
    double? prev;
    for (final e in elevations) {
      if (e == null || e.isNaN) continue;
      if (prev != null) {
        final d = e - prev;
        if (d > 0) {
          ascent += d;
        } else {
          descent += -d;
        }
      }
      prev = e;
    }
    return (ascent: ascent, descent: descent);
  }

  /// Project [position] onto the polyline and report progress along it.
  ///
  /// Uses a local equirectangular projection per segment (accurate at the
  /// scales hiking routes span). Returns null only when [points] has fewer than
  /// two vertices.
  static PathProgress? progress(List<LatLng> points, LatLng position) {
    if (points.length < 2) return null;

    // Cumulative length up to each vertex.
    final cumulative = List<double>.filled(points.length, 0);
    for (var i = 1; i < points.length; i++) {
      cumulative[i] = cumulative[i - 1] + _distance.distance(points[i - 1], points[i]);
    }
    final totalM = cumulative.last;

    double bestPerpM = double.infinity;
    int bestSeg = 0;
    LatLng bestSnap = points.first;
    double bestAlongM = 0;

    for (var i = 0; i < points.length - 1; i++) {
      final a = points[i];
      final b = points[i + 1];

      // Local metres relative to `a`.
      final mPerLng = _metresPerDegLng(a.latitude);
      final ax = 0.0, ay = 0.0;
      final bx = (b.longitude - a.longitude) * mPerLng;
      final by = (b.latitude - a.latitude) * _metresPerDegLat;
      final px = (position.longitude - a.longitude) * mPerLng;
      final py = (position.latitude - a.latitude) * _metresPerDegLat;

      final dx = bx - ax;
      final dy = by - ay;
      final segLenSq = dx * dx + dy * dy;
      double t = segLenSq == 0 ? 0 : ((px - ax) * dx + (py - ay) * dy) / segLenSq;
      t = t.clamp(0.0, 1.0);

      final snapX = ax + t * dx;
      final snapY = ay + t * dy;
      final perpM = math.sqrt((px - snapX) * (px - snapX) + (py - snapY) * (py - snapY));

      if (perpM < bestPerpM) {
        bestPerpM = perpM;
        bestSeg = i;
        // Convert the snapped local point back to lat/lng.
        bestSnap = LatLng(
          a.latitude + snapY / _metresPerDegLat,
          a.longitude + snapX / mPerLng,
        );
        final segLen = _distance.distance(a, b);
        bestAlongM = cumulative[i] + segLen * t;
      }
    }

    final remaining = (totalM - bestAlongM).clamp(0.0, totalM);
    final fraction = totalM == 0 ? 0.0 : (bestAlongM / totalM).clamp(0.0, 1.0);
    return PathProgress(
      segmentIndex: bestSeg,
      snapped: bestSnap,
      distanceAlongM: bestAlongM,
      remainingM: remaining,
      offRouteM: bestPerpM,
      fraction: fraction,
    );
  }

  /// How closely an [actual] track followed a [planned] route. Projects each
  /// actual point onto the planned line (reusing [progress]) and summarises:
  /// how far along the plan the walk reached, and the mean / worst perpendicular
  /// offset from it. Null when there isn't enough geometry to compare.
  static PathDeviation? deviation(List<LatLng> actual, List<LatLng> planned) {
    if (actual.isEmpty || planned.length < 2) return null;
    double offsetSum = 0;
    double maxOffset = 0;
    double maxFraction = 0;
    var counted = 0;
    for (final point in actual) {
      final prog = progress(planned, point);
      if (prog == null) continue;
      offsetSum += prog.offRouteM;
      if (prog.offRouteM > maxOffset) maxOffset = prog.offRouteM;
      if (prog.fraction > maxFraction) maxFraction = prog.fraction;
      counted++;
    }
    if (counted == 0) return null;
    return PathDeviation(
      completionFraction: maxFraction,
      avgOffsetM: offsetSum / counted,
      maxOffsetM: maxOffset,
    );
  }
}

/// Summary of how an actual track matched its planned route — see
/// [GeoMetrics.deviation].
class PathDeviation {
  /// 0‥1: how far along the planned route the track reached.
  final double completionFraction;

  /// Mean perpendicular offset of actual points from the planned line, metres.
  final double avgOffsetM;

  /// Worst perpendicular offset, metres.
  final double maxOffsetM;

  const PathDeviation({
    required this.completionFraction,
    required this.avgOffsetM,
    required this.maxOffsetM,
  });
}
