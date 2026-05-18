import 'package:uuid/uuid.dart';

/// A photo attached to a [Marker]. The photo bytes live on disk under the
/// app's documents directory at [filePath]; this object is the metadata that
/// links the file to its owning marker. The file lifecycle is owned by
/// `PhotoStorageService` — never delete files directly.
class MarkerPhoto {
  final String uuid;
  final String markerUuid;
  final String filePath;
  final DateTime createdAt;

  MarkerPhoto({
    String? uuid,
    required this.markerUuid,
    required this.filePath,
    DateTime? createdAt,
  })  : uuid = uuid ?? const Uuid().v4(),
        createdAt = createdAt ?? DateTime.now();

  factory MarkerPhoto.fromLocalMap(Map<String, dynamic> map) {
    return MarkerPhoto(
      uuid: map['uuid'] as String,
      markerUuid: map['marker_uuid'] as String,
      filePath: map['file_path'] as String,
      createdAt: DateTime.parse(map['created_at'] as String),
    );
  }

  Map<String, dynamic> toLocalMap() {
    return {
      'uuid': uuid,
      'marker_uuid': markerUuid,
      'file_path': filePath,
      'created_at': createdAt.toIso8601String(),
    };
  }

  @override
  bool operator ==(Object other) =>
      identical(this, other) ||
      other is MarkerPhoto &&
          uuid == other.uuid &&
          markerUuid == other.markerUuid &&
          filePath == other.filePath &&
          createdAt == other.createdAt;

  @override
  int get hashCode =>
      uuid.hashCode ^ markerUuid.hashCode ^ filePath.hashCode ^ createdAt.hashCode;
}
