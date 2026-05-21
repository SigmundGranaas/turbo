import 'package:latlong2/latlong.dart';

import 'freediving_details.dart';

class FreedivingActivity {
  final String id;
  final String ownerId;
  final String name;
  final String? description;
  final LatLng position;
  final FreedivingDetails details;
  final DateTime createdAt;
  final DateTime updatedAt;
  final int version;

  const FreedivingActivity({
    required this.id, required this.ownerId, required this.name, required this.description,
    required this.position, required this.details,
    required this.createdAt, required this.updatedAt, required this.version,
  });

  factory FreedivingActivity.fromJson(Map<String, dynamic> json) => FreedivingActivity(
        id: json['id'] as String, ownerId: json['ownerId'] as String,
        name: json['name'] as String, description: json['description'] as String?,
        position: LatLng((json['latitude'] as num).toDouble(), (json['longitude'] as num).toDouble()),
        details: FreedivingDetails.fromJson(json['details'] as Map<String, dynamic>),
        createdAt: DateTime.parse(json['createdAt'] as String),
        updatedAt: DateTime.parse(json['updatedAt'] as String),
        version: (json['version'] as num).toInt(),
      );
}
