import 'package:flutter_map/flutter_map.dart' show LatLngBounds;
import 'package:latlong2/latlong.dart';

import 'geo_metrics.dart';

/// Where a [GeoPath] came from. Lets consumers tailor presentation (e.g. a
/// recorded track vs. a freshly planned route) without re-introducing the
/// per-feature path types this value object replaces.
enum GeoPathSource { route, recording, measure, saved, trail, activity }

/// The canonical "a line on the map" value type.
///
/// Every feature that produces or consumes a polyline path (routing, recording,
/// measuring, saved paths, curated trails) converts to/from this single type.
/// It is the *only* path representation allowed to cross feature boundaries —
/// each feature exposes `toGeoPath()` from its own `api.dart`. This is the
/// composition seam that lets "follow this trail", "track this route" and
/// "save as track" all speak one language. See
/// `docs/architecture/2026-06-composition-overhaul-plan.md` (Phase 1).
class GeoPath {
  /// Ordered vertices, map order. May be empty.
  final List<LatLng> points;

  /// Optional per-point elevation in metres. Length matches [points] when
  /// present; individual entries are null where no fix/sample existed. This is
  /// the canonical nullable-per-point form — `SavedPath` stores the NaN-filled
  /// variant and converts on the boundary.
  final List<double?>? elevations;

  /// Total length, metres. Authoritative when supplied by the producer
  /// (server-solved routes, recorded tracks); otherwise derive with
  /// [GeoPath.fromPoints].
  final double distanceM;

  /// Total positive ascent, metres. Null when unknown (no DEM/elevation).
  final double? ascentM;

  /// Total descent, metres. Null when unknown.
  final double? descentM;

  /// Actual moving time in seconds (recorded tracks). Null for planned/
  /// measured paths — a route's *estimated* duration is not moving time.
  final int? movingTimeSeconds;

  /// When the path was recorded (recorded/saved tracks). Null otherwise.
  final DateTime? recordedAt;

  final GeoPathSource source;

  const GeoPath({
    required this.points,
    required this.distanceM,
    this.elevations,
    this.ascentM,
    this.descentM,
    this.movingTimeSeconds,
    this.recordedAt,
    required this.source,
  });

  /// Build from raw points, computing [distanceM] with the shared metrics
  /// engine when the producer doesn't already know it.
  factory GeoPath.fromPoints(
    List<LatLng> points, {
    required GeoPathSource source,
    List<double?>? elevations,
    double? ascentM,
    double? descentM,
    int? movingTimeSeconds,
    DateTime? recordedAt,
  }) {
    return GeoPath(
      points: points,
      distanceM: GeoMetrics.pathLengthMeters(points),
      elevations: elevations,
      ascentM: ascentM,
      descentM: descentM,
      movingTimeSeconds: movingTimeSeconds,
      recordedAt: recordedAt,
      source: source,
    );
  }

  bool get isEmpty => points.length < 2;
  bool get isNotEmpty => !isEmpty;

  /// Axis-aligned bounding box. Throws if [points] is empty — guard with
  /// [isEmpty] first (mirrors `SavedPath.bounds`).
  LatLngBounds get bounds {
    double minLat = points.first.latitude;
    double maxLat = points.first.latitude;
    double minLng = points.first.longitude;
    double maxLng = points.first.longitude;
    for (final p in points) {
      if (p.latitude < minLat) minLat = p.latitude;
      if (p.latitude > maxLat) maxLat = p.latitude;
      if (p.longitude < minLng) minLng = p.longitude;
      if (p.longitude > maxLng) maxLng = p.longitude;
    }
    return LatLngBounds(LatLng(minLat, minLng), LatLng(maxLat, maxLng));
  }

  GeoPath copyWith({
    List<LatLng>? points,
    List<double?>? elevations,
    double? distanceM,
    double? ascentM,
    int? movingTimeSeconds,
    double? descentM,
    DateTime? recordedAt,
    GeoPathSource? source,
  }) {
    return GeoPath(
      points: points ?? this.points,
      elevations: elevations ?? this.elevations,
      distanceM: distanceM ?? this.distanceM,
      ascentM: ascentM ?? this.ascentM,
      descentM: descentM ?? this.descentM,
      movingTimeSeconds: movingTimeSeconds ?? this.movingTimeSeconds,
      recordedAt: recordedAt ?? this.recordedAt,
      source: source ?? this.source,
    );
  }
}
