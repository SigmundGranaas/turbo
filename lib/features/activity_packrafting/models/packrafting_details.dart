import 'package:latlong2/latlong.dart';

import 'package:turbo/features/activities/api.dart' show ActivityGeometry;

enum WaterGrade { flatwater, i, ii, iii, iv, v, vi }
enum SegmentKind { paddle, portage }

class PackraftingDetails {
  final int distanceMeters;
  final int paddleDistanceMeters;
  final int portageDistanceMeters;
  final WaterGrade maxGrade;
  final WaterGrade typicalGrade;
  final LatLng putIn;
  final LatLng takeOut;
  final String? nveStationCode;
  final double? minFlowCumecs;
  final double? maxFlowCumecs;
  final List<RouteSegment> segments;

  const PackraftingDetails({
    required this.distanceMeters,
    required this.paddleDistanceMeters,
    required this.portageDistanceMeters,
    required this.maxGrade,
    required this.typicalGrade,
    required this.putIn,
    required this.takeOut,
    this.nveStationCode,
    this.minFlowCumecs,
    this.maxFlowCumecs,
    this.segments = const [],
  });

  Map<String, dynamic> toJson() => {
        'distanceMeters': distanceMeters,
        'paddleDistanceMeters': paddleDistanceMeters,
        'portageDistanceMeters': portageDistanceMeters,
        'maxGrade': maxGrade.index,
        'typicalGrade': typicalGrade.index,
        'putInLat': putIn.latitude, 'putInLon': putIn.longitude,
        'takeOutLat': takeOut.latitude, 'takeOutLon': takeOut.longitude,
        'nveStationCode': ?nveStationCode,
        'minFlowCumecs': ?minFlowCumecs,
        'maxFlowCumecs': ?maxFlowCumecs,
        'segments': segments.map((s) => s.toJson()).toList(),
      };

  factory PackraftingDetails.fromJson(Map<String, dynamic> json) => PackraftingDetails(
        distanceMeters: (json['distanceMeters'] as num).toInt(),
        paddleDistanceMeters: (json['paddleDistanceMeters'] as num).toInt(),
        portageDistanceMeters: (json['portageDistanceMeters'] as num).toInt(),
        maxGrade: WaterGrade.values[(json['maxGrade'] as num).toInt()],
        typicalGrade: WaterGrade.values[(json['typicalGrade'] as num).toInt()],
        putIn: LatLng((json['putInLat'] as num).toDouble(), (json['putInLon'] as num).toDouble()),
        takeOut: LatLng((json['takeOutLat'] as num).toDouble(), (json['takeOutLon'] as num).toDouble()),
        nveStationCode: json['nveStationCode'] as String?,
        minFlowCumecs: (json['minFlowCumecs'] as num?)?.toDouble(),
        maxFlowCumecs: (json['maxFlowCumecs'] as num?)?.toDouble(),
        segments: (json['segments'] as List?)
                ?.cast<Map<String, dynamic>>()
                .map(RouteSegment.fromJson)
                .toList() ?? const [],
      );
}

class RouteSegment {
  final SegmentKind kind;
  final WaterGrade? grade;
  final int distanceMeters;
  final List<LatLng> polyline;
  final String? notes;

  const RouteSegment({
    required this.kind, this.grade,
    required this.distanceMeters,
    required this.polyline,
    this.notes,
  });

  Map<String, dynamic> toJson() => {
        'kind': kind.index,
        'grade': ?grade?.index,
        'distanceMeters': distanceMeters,
        'polylineWkt': _wkt(polyline),
        'notes': ?notes,
      };

  factory RouteSegment.fromJson(Map<String, dynamic> json) {
    final geom = ActivityGeometry.fromServer(wkt: json['polylineWkt'] as String, geometryKind: 'LINESTRING');
    return RouteSegment(
      kind: SegmentKind.values[(json['kind'] as num).toInt()],
      grade: json['grade'] == null ? null : WaterGrade.values[(json['grade'] as num).toInt()],
      distanceMeters: (json['distanceMeters'] as num).toInt(),
      polyline: geom.coordinates,
      notes: json['notes'] as String?,
    );
  }

  static String _wkt(List<LatLng> points) {
    if (points.isEmpty) return 'LINESTRING EMPTY';
    return 'LINESTRING(${points.map((p) => '${p.longitude} ${p.latitude}').join(', ')})';
  }
}
