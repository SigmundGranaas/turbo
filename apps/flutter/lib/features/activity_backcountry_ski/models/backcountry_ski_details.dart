import 'package:latlong2/latlong.dart';

import 'package:turbo/features/activities/api.dart' show ActivityGeometry;

/// ATES rating mirroring the server enum.
enum AtesRating { unrated, simple, challenging, complex }

/// 8-point cardinal aspect.
enum Aspect { n, ne, e, se, s, sw, w, nw }

/// Leg of the route.
enum LegKind { ascent, descent, traverse }

class BackcountrySkiDetails {
  final int ascentMeters;
  final int descentMeters;
  final int distanceMeters;
  final int elevationMinMeters;
  final int elevationMaxMeters;
  final AtesRating atesRating;
  final Aspect? dominantAspect;
  final int? varsomRegionId;
  final int? preferredAvalancheMaxLevel;
  final List<AspectShare> aspectMix;
  final List<RouteLeg> legs;

  const BackcountrySkiDetails({
    required this.ascentMeters,
    required this.descentMeters,
    required this.distanceMeters,
    required this.elevationMinMeters,
    required this.elevationMaxMeters,
    required this.atesRating,
    this.dominantAspect,
    this.varsomRegionId,
    this.preferredAvalancheMaxLevel,
    this.aspectMix = const [],
    this.legs = const [],
  });

  Map<String, dynamic> toJson() => {
        'ascentMeters': ascentMeters,
        'descentMeters': descentMeters,
        'distanceMeters': distanceMeters,
        'elevationMinMeters': elevationMinMeters,
        'elevationMaxMeters': elevationMaxMeters,
        'atesRating': atesRating.index,
        'dominantAspect': ?dominantAspect?.index,
        'varsomRegionId': ?varsomRegionId,
        'preferredAvalancheMaxLevel': ?preferredAvalancheMaxLevel,
        'aspectMix': aspectMix.map((a) => a.toJson()).toList(),
        'legs': legs.map((l) => l.toJson()).toList(),
      };

  factory BackcountrySkiDetails.fromJson(Map<String, dynamic> json) =>
      BackcountrySkiDetails(
        ascentMeters: (json['ascentMeters'] as num).toInt(),
        descentMeters: (json['descentMeters'] as num).toInt(),
        distanceMeters: (json['distanceMeters'] as num).toInt(),
        elevationMinMeters: (json['elevationMinMeters'] as num).toInt(),
        elevationMaxMeters: (json['elevationMaxMeters'] as num).toInt(),
        atesRating: AtesRating.values[(json['atesRating'] as num).toInt()],
        dominantAspect: json['dominantAspect'] == null
            ? null
            : Aspect.values[(json['dominantAspect'] as num).toInt()],
        varsomRegionId: (json['varsomRegionId'] as num?)?.toInt(),
        preferredAvalancheMaxLevel:
            (json['preferredAvalancheMaxLevel'] as num?)?.toInt(),
        aspectMix: (json['aspectMix'] as List?)
                ?.cast<Map<String, dynamic>>()
                .map(AspectShare.fromJson)
                .toList() ??
            const [],
        legs: (json['legs'] as List?)
                ?.cast<Map<String, dynamic>>()
                .map(RouteLeg.fromJson)
                .toList() ??
            const [],
      );
}

class AspectShare {
  final Aspect aspect;
  final double fraction;
  const AspectShare({required this.aspect, required this.fraction});

  Map<String, dynamic> toJson() => {
        'aspect': aspect.index,
        'fraction': fraction,
      };

  factory AspectShare.fromJson(Map<String, dynamic> json) => AspectShare(
        aspect: Aspect.values[(json['aspect'] as num).toInt()],
        fraction: (json['fraction'] as num).toDouble(),
      );
}

class RouteLeg {
  final LegKind kind;
  final int startElevationMeters;
  final int endElevationMeters;
  final List<LatLng> polyline;
  const RouteLeg({
    required this.kind,
    required this.startElevationMeters,
    required this.endElevationMeters,
    required this.polyline,
  });

  Map<String, dynamic> toJson() => {
        'kind': kind.index,
        'startElevationMeters': startElevationMeters,
        'endElevationMeters': endElevationMeters,
        'polylineWkt': _toWkt(polyline),
      };

  factory RouteLeg.fromJson(Map<String, dynamic> json) {
    final wkt = json['polylineWkt'] as String;
    final geom = ActivityGeometry.fromServer(wkt: wkt, geometryKind: 'LINESTRING');
    return RouteLeg(
      kind: LegKind.values[(json['kind'] as num).toInt()],
      startElevationMeters: (json['startElevationMeters'] as num).toInt(),
      endElevationMeters: (json['endElevationMeters'] as num).toInt(),
      polyline: geom.coordinates,
    );
  }

  static String _toWkt(List<LatLng> points) {
    if (points.isEmpty) return 'LINESTRING EMPTY';
    final pairs =
        points.map((p) => '${p.longitude} ${p.latitude}').join(', ');
    return 'LINESTRING($pairs)';
  }
}
