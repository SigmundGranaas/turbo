import 'package:latlong2/latlong.dart';
import 'package:uuid/uuid.dart';

class Marker {
  final String uuid;
  final LatLng position;
  final String title;
  final String? description;
  final String? icon;

  Marker({
    String? uuid,
    required this.position,
    required this.title,
    this.description,
    this.icon,
  }) : uuid = uuid ?? const Uuid().v4();

  Map<String, dynamic> toMap() {
    return {
      'uuid': uuid,
      'latitude': position.latitude,
      'longitude': position.longitude,
      'title': title,
      'description': description,
      'icon': icon,
    };
  }

  factory Marker.fromMap(Map<String, dynamic> map) {
    return Marker(
      uuid: map['uuid'],
      position: LatLng(map['latitude'], map['longitude']),
      title: map['title'],
      description: map['description'],
      icon: map['icon'],
    );
  }
}