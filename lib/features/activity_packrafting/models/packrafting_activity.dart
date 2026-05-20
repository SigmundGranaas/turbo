import 'package:latlong2/latlong.dart';

import 'package:turbo/features/activities/api.dart' show ActivityGeometry;
import 'packrafting_details.dart';

class PackraftingActivity {
  final String id;
  final String ownerId;
  final String name;
  final String? description;
  final List<LatLng> route;
  final PackraftingDetails details;
  final DateTime createdAt;
  final DateTime updatedAt;
  final int version;

  const PackraftingActivity({
    required this.id, required this.ownerId, required this.name, required this.description,
    required this.route, required this.details,
    required this.createdAt, required this.updatedAt, required this.version,
  });

  factory PackraftingActivity.fromJson(Map<String, dynamic> json) {
    final geom = ActivityGeometry.fromServer(wkt: json['routeWkt'] as String, geometryKind: 'LINESTRING');
    return PackraftingActivity(
      id: json['id'] as String, ownerId: json['ownerId'] as String,
      name: json['name'] as String, description: json['description'] as String?,
      route: geom.coordinates,
      details: PackraftingDetails.fromJson(json['details'] as Map<String, dynamic>),
      createdAt: DateTime.parse(json['createdAt'] as String),
      updatedAt: DateTime.parse(json['updatedAt'] as String),
      version: (json['version'] as num).toInt(),
    );
  }
}
