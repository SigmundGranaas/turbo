/// Domain models for the Routing feature — the wire contract of the
/// curated routing API (`/api/route/*` → tileserver `/v1/route/*`; see
/// `apps/tileserver/docs/route-api.openapi.yaml`).
///
/// Value-only: no HTTP, no providers. The data layer passes these around
/// freely and the contract is deliberately decoupled from the solver
/// internals on the server side.
library;

import 'package:latlong2/latlong.dart';

/// A trip-style preset (see `GET .../presets`). `name` is the wire id
/// sent back in a [RouteRequest]; `label`/`description` are for display.
class RoutePreset {
  final String name;
  final String label;
  final String description;

  const RoutePreset({
    required this.name,
    required this.label,
    required this.description,
  });

  factory RoutePreset.fromJson(Map<String, dynamic> json) => RoutePreset(
        name: json['name'] as String,
        label: json['label'] as String,
        description: json['description'] as String,
      );
}

/// Request to plan a route through an ordered list of waypoints.
class RouteRequest {
  /// Ordered waypoints (start, intermediate vias, end), visited in order.
  /// Must contain at least 2 points.
  final List<LatLng> points;

  /// Preset name (see [RoutePreset.name]). `null` → server default
  /// ("balanced").
  final String? preset;

  /// Travel profile: "foot" (default), "bicycle", or "ski". `null` →
  /// server default. Only "foot" is calibrated today.
  final String? profile;

  const RouteRequest({required this.points, this.preset, this.profile});

  Map<String, dynamic> toJson() => {
        // The wire uses GeoJSON axis order: [lon, lat].
        'points': points.map((p) => [p.longitude, p.latitude]).toList(),
        if (preset != null) 'preset': preset,
        if (profile != null) 'profile': profile,
      };
}

/// One inter-waypoint leg of a planned route. Indices point into the
/// request's `points` list.
class RouteLeg {
  final int fromIndex;
  final int toIndex;
  final double distanceM;

  const RouteLeg({
    required this.fromIndex,
    required this.toIndex,
    required this.distanceM,
  });

  factory RouteLeg.fromJson(Map<String, dynamic> json) => RouteLeg(
        fromIndex: (json['from_index'] as num).toInt(),
        toIndex: (json['to_index'] as num).toInt(),
        distanceM: (json['distance_m'] as num).toDouble(),
      );
}

/// A solved route — the stable response contract from `POST .../plan`.
class RoutePlan {
  /// Total route length, metres.
  final double distanceM;

  /// Estimated travel time (Naismith-style: flat pace + ascent), seconds.
  final double durationS;

  /// Total positive ascent, metres (0 when no DEM coverage).
  final double ascentM;

  /// Percent of the route on marked trails (0–100).
  final double onTrailPct;

  /// Metres by surface. Keys: `trail`, `road`, `ski_track`, `off_trail`,
  /// `unknown`. Only surfaces actually present are included.
  final Map<String, double> surfaces;

  /// The route polyline in map order (already lon/lat → [LatLng]).
  final List<LatLng> geometry;

  /// Per-waypoint-leg summary (empty for a simple 2-point route).
  final List<RouteLeg> legs;

  const RoutePlan({
    required this.distanceM,
    required this.durationS,
    required this.ascentM,
    required this.onTrailPct,
    required this.surfaces,
    required this.geometry,
    required this.legs,
  });

  /// Estimated travel time as a [Duration].
  Duration get duration => Duration(seconds: durationS.round());

  factory RoutePlan.fromJson(Map<String, dynamic> json) {
    final geom = json['geometry'] as Map<String, dynamic>?;
    final coords = (geom?['coordinates'] as List?) ?? const [];
    return RoutePlan(
      distanceM: (json['distance_m'] as num).toDouble(),
      durationS: (json['duration_s'] as num).toDouble(),
      ascentM: (json['ascent_m'] as num).toDouble(),
      onTrailPct: (json['on_trail_pct'] as num).toDouble(),
      surfaces: ((json['surfaces'] as Map?) ?? const {}).map(
        (k, v) => MapEntry(k as String, (v as num).toDouble()),
      ),
      geometry: coords.map((c) {
        // GeoJSON [lon, lat] → LatLng(lat, lon).
        final pair = c as List;
        return LatLng(
          (pair[1] as num).toDouble(),
          (pair[0] as num).toDouble(),
        );
      }).toList(growable: false),
      legs: ((json['legs'] as List?) ?? const [])
          .map((e) => RouteLeg.fromJson(e as Map<String, dynamic>))
          .toList(growable: false),
    );
  }
}
