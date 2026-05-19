import 'package:latlong2/latlong.dart';
import 'package:uuid/uuid.dart';

class Marker {
  final String uuid;
  final String title;
  final String? description;
  final String? icon;
  final LatLng position;
  final bool synced;

  /// Server-stamped monotonic row version. Used as the ETag for
  /// optimistic concurrency (sent back as `If-Match` on update/delete).
  /// Null when the marker has not yet been synced to the server.
  final int? version;

  /// Server-stamped wall-clock for the last successful projection write.
  /// Used to compute the next `?since=` cursor for delta-sync.
  final DateTime? updatedAt;

  /// Server-side tombstone. The client uses this to recognise deletions
  /// learnt via delta-sync (versus a 404 / row-missing). Always null in
  /// the local store for non-tombstoned rows.
  final DateTime? deletedAt;

  Marker({
    String? uuid,
    required this.title,
    this.description,
    this.icon,
    required this.position,
    this.synced = false,
    this.version,
    this.updatedAt,
    this.deletedAt,
  }) : uuid = uuid ?? const Uuid().v4();

  Marker copyWith({
    String? uuid,
    String? title,
    String? description,
    String? icon,
    LatLng? position,
    bool? synced,
    int? version,
    bool clearVersion = false,
    DateTime? updatedAt,
    bool clearUpdatedAt = false,
    DateTime? deletedAt,
    bool clearDeletedAt = false,
  }) {
    return Marker(
      uuid: uuid ?? this.uuid,
      title: title ?? this.title,
      description: description ?? this.description,
      icon: icon ?? this.icon,
      position: position ?? this.position,
      synced: synced ?? this.synced,
      version: clearVersion ? null : (version ?? this.version),
      updatedAt: clearUpdatedAt ? null : (updatedAt ?? this.updatedAt),
      deletedAt: clearDeletedAt ? null : (deletedAt ?? this.deletedAt),
    );
  }

  factory Marker.fromLocalMap(Map<String, dynamic> map) {
    DateTime? parseOptional(dynamic raw) {
      if (raw is String && raw.isNotEmpty) return DateTime.parse(raw);
      return null;
    }

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
      version: (map['version'] as num?)?.toInt(),
      updatedAt: parseOptional(map['updated_at']),
      deletedAt: parseOptional(map['deleted_at']),
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
      'version': version,
      'updated_at': updatedAt?.toIso8601String(),
      'deleted_at': deletedAt?.toIso8601String(),
    };
  }

  factory Marker.fromApiResponse(Map<String, dynamic> responseData) {
    final geometry = responseData['geometry'] as Map<String, dynamic>;
    final display = responseData['display'] as Map<String, dynamic>;
    DateTime? parseOptional(dynamic raw) {
      if (raw is String && raw.isNotEmpty) return DateTime.parse(raw);
      return null;
    }

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
      version: (responseData['version'] as num?)?.toInt(),
      updatedAt: parseOptional(responseData['updatedAt']),
      deletedAt: parseOptional(responseData['deletedAt']),
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
              synced == other.synced &&
              version == other.version &&
              updatedAt == other.updatedAt &&
              deletedAt == other.deletedAt;

  @override
  int get hashCode =>
      uuid.hashCode ^
      title.hashCode ^
      description.hashCode ^
      icon.hashCode ^
      position.hashCode ^
      synced.hashCode ^
      version.hashCode ^
      updatedAt.hashCode ^
      deletedAt.hashCode;
}