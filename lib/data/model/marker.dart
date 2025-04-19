import 'package:latlong2/latlong.dart';
import 'package:uuid/uuid.dart';

class Marker {
  final String uuid;
  final String title;
  final String description;
  final String icon;
  final LatLng position;
  final bool synced;

  Marker({
    String? uuid,
    required this.title,
    this.description = '',
    this.icon = '',
    required this.position,
    this.synced = false,
  }) : uuid = uuid ?? const Uuid().v4();

  // Factory constructor for creating an empty marker
  factory Marker.empty() {
    return Marker(
      uuid: '',
      title: '',
      position: const LatLng(0, 0),
    );
  }

  // Create a copy of this marker with modified properties
  Marker copyWith({
    String? uuid,
    String? title,
    String? description,
    String? icon,
    LatLng? position,
    bool? synced,
  }) {
    return Marker(
      uuid: uuid ?? this.uuid,
      title: title ?? this.title,
      description: description ?? this.description,
      icon: icon ?? this.icon,
      position: position ?? this.position,
      synced: synced ?? this.synced,
    );
  }

  // Factory constructor for creating from map (used by your UI)
  factory Marker.fromMap(Map<String, dynamic> map) {
    return Marker(
      uuid: map['uuid'],
      title: map['title'] ?? '',
      description: map['description'] ?? '',
      icon: map['icon'] ?? '',
      position: LatLng(
        map['latitude'] ?? 0.0,
        map['longitude'] ?? 0.0,
      ),
    );
  }

  // For serialization to local storage
  Map<String, dynamic> toMap() {
    return {
      'uuid': uuid,
      'title': title,
      'description': description,
      'icon': icon,
      'latitude': position.latitude,
      'longitude': position.longitude,
      'synced': synced ? 1 : 0,
    };
  }

  // For deserialization from local storage
  factory Marker.fromJson(Map<String, dynamic> json) {
    return Marker(
      uuid: json['uuid'],
      title: json['title'] ?? '',
      description: json['description'] ?? '',
      icon: json['icon'] ?? '',
      position: LatLng(json['latitude'], json['longitude']),
      synced: json['synced'] == 1 || json['synced'] == true,
    );
  }
}