import 'package:latlong2/latlong.dart';
import 'package:uuid/uuid.dart';

class Marker {
  final String uuid;
  final String title;
  final String? description;
  final String? icon;
  final LatLng position;
  final bool synced;

  Marker({
    String? uuid,
    required this.title,
    this.description,
    this.icon,
    required this.position,
    this.synced = false,
  }) : uuid = uuid ?? const Uuid().v4();

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

  factory Marker.fromLocalMap(Map<String, dynamic> map) {
    return Marker(
      uuid: map['uuid'] as String,
      title: map['title'] as String,
      description: map['description'] as String?,
      icon: map['icon'] as String?,
      position: LatLng(
        map['latitude'] as double,
        map['longitude'] as double,
      ),
      synced: (map['synced'] == 1 || map['synced'] == true), // Handles int (SQLite) and bool (IndexedDB)
    );
  }

  Map<String, dynamic> toLocalMap() {
    return {
      'uuid': uuid,
      'title': title,
      'description': description,
      'icon': icon,
      'latitude': position.latitude,
      'longitude': position.longitude,
      'synced': synced ? 1 : 0, // Store as int for SQLite compatibility
    };
  }

  factory Marker.fromApiResponse(Map<String, dynamic> responseData) {
    final geometry = responseData['geometry'] as Map<String, dynamic>;
    final display = responseData['display'] as Map<String, dynamic>;
    return Marker(
      uuid: responseData['id'] as String,
      title: display['name'] as String,
      description: display['description'] as String?,
      icon: display['icon'] as String?,
      position: LatLng(
        geometry['latitude'] as double,
        geometry['longitude'] as double,
      ),
      synced: true,
    );
  }

  @override
  bool operator ==(Object other) =>
      identical(this, other) ||
          other is Marker &&
              runtimeType == other.runtimeType &&
              uuid == other.uuid &&
              title == other.title &&
              description == other.description &&
              icon == other.icon &&
              position == other.position &&
              synced == other.synced;

  @override
  int get hashCode =>
      uuid.hashCode ^
      title.hashCode ^
      description.hashCode ^
      icon.hashCode ^
      position.hashCode ^
      synced.hashCode;
}