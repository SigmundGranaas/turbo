import 'package:latlong2/latlong.dart';

import 'package:turbo/features/activities/api.dart' show ActivityGeometry;

import 'backcountry_ski_details.dart';

/// Client-side mirror of the server's typed
/// /api/activities/backcountry-ski/{id} response. Composes the typed
/// details payload — no map-of-dynamic catch-all.
class BackcountrySkiActivity {
  final String id;
  final String ownerId;
  final String name;
  final String? description;
  final List<LatLng> route;
  final BackcountrySkiDetails details;
  final DateTime createdAt;
  final DateTime updatedAt;
  final int version;

  const BackcountrySkiActivity({
    required this.id,
    required this.ownerId,
    required this.name,
    required this.description,
    required this.route,
    required this.details,
    required this.createdAt,
    required this.updatedAt,
    required this.version,
  });

  factory BackcountrySkiActivity.fromJson(Map<String, dynamic> json) {
    final routeWkt = json['routeWkt'] as String;
    final route = ActivityGeometry.fromServer(
      wkt: routeWkt,
      geometryKind: 'LINESTRING',
    );
    return BackcountrySkiActivity(
      id: json['id'] as String,
      ownerId: json['ownerId'] as String,
      name: json['name'] as String,
      description: json['description'] as String?,
      route: route.coordinates,
      details: BackcountrySkiDetails.fromJson(json['details'] as Map<String, dynamic>),
      createdAt: DateTime.parse(json['createdAt'] as String),
      updatedAt: DateTime.parse(json['updatedAt'] as String),
      version: (json['version'] as num).toInt(),
    );
  }
}
