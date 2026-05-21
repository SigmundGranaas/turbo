import 'package:latlong2/latlong.dart';

import 'fishing_details.dart';

/// Client-side representation of one fishing activity. Round-trips with
/// the server's typed `/api/activities/fishing/{id}` endpoint. The model
/// mirrors the server: an immutable record of identity, ownership,
/// position, naming, plus the typed [FishingDetails] payload.
class FishingActivity {
  final String id;
  final String ownerId;
  final String name;
  final String? description;
  final LatLng position;
  final FishingDetails details;
  final DateTime createdAt;
  final DateTime updatedAt;
  final int version;

  const FishingActivity({
    required this.id,
    required this.ownerId,
    required this.name,
    required this.description,
    required this.position,
    required this.details,
    required this.createdAt,
    required this.updatedAt,
    required this.version,
  });

  factory FishingActivity.fromJson(Map<String, dynamic> json) => FishingActivity(
        id: json['id'] as String,
        ownerId: json['ownerId'] as String,
        name: json['name'] as String,
        description: json['description'] as String?,
        position: LatLng(
          (json['latitude'] as num).toDouble(),
          (json['longitude'] as num).toDouble(),
        ),
        details: FishingDetails.fromJson(json['details'] as Map<String, dynamic>),
        createdAt: DateTime.parse(json['createdAt'] as String),
        updatedAt: DateTime.parse(json['updatedAt'] as String),
        version: (json['version'] as num).toInt(),
      );
}
