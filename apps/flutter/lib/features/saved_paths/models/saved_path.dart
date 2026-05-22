import 'dart:convert';

import 'package:flutter_map/flutter_map.dart' show LatLngBounds;
import 'package:latlong2/latlong.dart';
import 'package:uuid/uuid.dart';

class SavedPath {
  final String uuid;
  final String title;
  final String? description;
  final List<LatLng> points;
  final double distance;
  final DateTime createdAt;
  final String? colorHex;
  final String? iconKey;
  final bool smoothing;
  final String? lineStyleKey;
  final List<double>? elevations;
  final DateTime? recordedAt;
  final double? ascent;
  final double? descent;
  final int? movingTimeSeconds;

  /// Sync state: true once the row has been successfully written to the
  /// server's read model. Locally-created paths default to false; the
  /// sync orchestrator flips this to true after upload.
  final bool synced;

  /// Server-stamped monotonic version. Sent back as `If-Match` on
  /// update/delete; null while the track has not yet synced.
  final int? version;

  /// Server-stamped wall-clock of the last successful projection write.
  /// Drives the next `?since=` cursor for delta-sync.
  final DateTime? updatedAt;

  /// Server-side tombstone. The client uses this to recognise deletions
  /// learnt via delta-sync; always null in the local store for live rows.
  final DateTime? deletedAt;

  SavedPath({
    String? uuid,
    required this.title,
    this.description,
    required this.points,
    required this.distance,
    DateTime? createdAt,
    this.colorHex,
    this.iconKey,
    this.smoothing = false,
    this.lineStyleKey,
    this.elevations,
    this.recordedAt,
    this.ascent,
    this.descent,
    this.movingTimeSeconds,
    this.synced = false,
    this.version,
    this.updatedAt,
    this.deletedAt,
  })  : uuid = uuid ?? const Uuid().v4(),
        createdAt = createdAt ?? DateTime.now();

  SavedPath copyWith({
    String? uuid,
    String? title,
    String? description,
    List<LatLng>? points,
    double? distance,
    DateTime? createdAt,
    String? colorHex,
    bool clearColorHex = false,
    String? iconKey,
    bool clearIconKey = false,
    bool? smoothing,
    String? lineStyleKey,
    bool clearLineStyleKey = false,
    List<double>? elevations,
    bool clearElevations = false,
    DateTime? recordedAt,
    bool clearRecordedAt = false,
    double? ascent,
    bool clearAscent = false,
    double? descent,
    bool clearDescent = false,
    int? movingTimeSeconds,
    bool clearMovingTimeSeconds = false,
    bool? synced,
    int? version,
    bool clearVersion = false,
    DateTime? updatedAt,
    bool clearUpdatedAt = false,
    DateTime? deletedAt,
    bool clearDeletedAt = false,
  }) {
    return SavedPath(
      uuid: uuid ?? this.uuid,
      title: title ?? this.title,
      description: description ?? this.description,
      points: points ?? this.points,
      distance: distance ?? this.distance,
      createdAt: createdAt ?? this.createdAt,
      colorHex: clearColorHex ? null : (colorHex ?? this.colorHex),
      iconKey: clearIconKey ? null : (iconKey ?? this.iconKey),
      smoothing: smoothing ?? this.smoothing,
      lineStyleKey: clearLineStyleKey ? null : (lineStyleKey ?? this.lineStyleKey),
      elevations: clearElevations ? null : (elevations ?? this.elevations),
      recordedAt: clearRecordedAt ? null : (recordedAt ?? this.recordedAt),
      ascent: clearAscent ? null : (ascent ?? this.ascent),
      descent: clearDescent ? null : (descent ?? this.descent),
      movingTimeSeconds:
          clearMovingTimeSeconds ? null : (movingTimeSeconds ?? this.movingTimeSeconds),
      synced: synced ?? this.synced,
      version: clearVersion ? null : (version ?? this.version),
      updatedAt: clearUpdatedAt ? null : (updatedAt ?? this.updatedAt),
      deletedAt: clearDeletedAt ? null : (deletedAt ?? this.deletedAt),
    );
  }

  LatLngBounds get bounds {
    double minLat = points.first.latitude;
    double maxLat = points.first.latitude;
    double minLng = points.first.longitude;
    double maxLng = points.first.longitude;

    for (final p in points) {
      if (p.latitude < minLat) minLat = p.latitude;
      if (p.latitude > maxLat) maxLat = p.latitude;
      if (p.longitude < minLng) minLng = p.longitude;
      if (p.longitude > maxLng) maxLng = p.longitude;
    }

    return LatLngBounds(LatLng(minLat, minLng), LatLng(maxLat, maxLng));
  }

  factory SavedPath.fromLocalMap(Map<String, dynamic> map) {
    final pointsJson = jsonDecode(map['points'] as String) as List;
    final points = pointsJson
        .map((p) => LatLng((p as List)[0] as double, p[1] as double))
        .toList();

    List<double>? elevations;
    final elevRaw = map['elevations'];
    if (elevRaw is String && elevRaw.isNotEmpty) {
      final decoded = jsonDecode(elevRaw) as List;
      elevations = decoded.map((e) => (e as num).toDouble()).toList();
    }

    DateTime? recordedAt;
    final recRaw = map['recorded_at'];
    if (recRaw is String && recRaw.isNotEmpty) {
      recordedAt = DateTime.parse(recRaw);
    }

    DateTime? parseOptional(dynamic raw) {
      if (raw is String && raw.isNotEmpty) return DateTime.parse(raw);
      return null;
    }

    return SavedPath(
      uuid: map['uuid'] as String,
      title: map['title'] as String,
      description: map['description'] as String?,
      points: points,
      distance: (map['distance'] as num).toDouble(),
      createdAt: DateTime.parse(map['created_at'] as String),
      colorHex: map['color_hex'] as String?,
      iconKey: map['icon_key'] as String?,
      smoothing: map['smoothing'] == 1 || map['smoothing'] == true,
      lineStyleKey: map['line_style'] as String?,
      elevations: elevations,
      recordedAt: recordedAt,
      ascent: (map['ascent'] as num?)?.toDouble(),
      descent: (map['descent'] as num?)?.toDouble(),
      movingTimeSeconds: (map['moving_time_seconds'] as num?)?.toInt(),
      synced: map['synced'] == 1 || map['synced'] == true,
      version: (map['version'] as num?)?.toInt(),
      updatedAt: parseOptional(map['updated_at']),
      deletedAt: parseOptional(map['deleted_at']),
    );
  }

  Map<String, dynamic> toLocalMap() {
    final b = bounds;
    return {
      'uuid': uuid,
      'title': title,
      'description': description,
      'points': jsonEncode(points.map((p) => [p.latitude, p.longitude]).toList()),
      'distance': distance,
      'min_lat': b.southWest.latitude,
      'min_lng': b.southWest.longitude,
      'max_lat': b.northEast.latitude,
      'max_lng': b.northEast.longitude,
      'created_at': createdAt.toIso8601String(),
      'color_hex': colorHex,
      'icon_key': iconKey,
      'smoothing': smoothing ? 1 : 0,
      'line_style': lineStyleKey,
      'elevations': elevations == null ? null : jsonEncode(elevations),
      'recorded_at': recordedAt?.toIso8601String(),
      'ascent': ascent,
      'descent': descent,
      'moving_time_seconds': movingTimeSeconds,
      'synced': synced ? 1 : 0,
      'version': version,
      'updated_at': updatedAt?.toIso8601String(),
      'deleted_at': deletedAt?.toIso8601String(),
    };
  }

  @override
  bool operator ==(Object other) =>
      identical(this, other) ||
      other is SavedPath &&
          runtimeType == other.runtimeType &&
          uuid == other.uuid &&
          title == other.title &&
          description == other.description &&
          distance == other.distance &&
          createdAt == other.createdAt &&
          colorHex == other.colorHex &&
          iconKey == other.iconKey &&
          smoothing == other.smoothing &&
          lineStyleKey == other.lineStyleKey &&
          recordedAt == other.recordedAt &&
          ascent == other.ascent &&
          descent == other.descent &&
          movingTimeSeconds == other.movingTimeSeconds;

  @override
  int get hashCode =>
      uuid.hashCode ^
      title.hashCode ^
      description.hashCode ^
      distance.hashCode ^
      createdAt.hashCode ^
      colorHex.hashCode ^
      iconKey.hashCode ^
      smoothing.hashCode ^
      lineStyleKey.hashCode ^
      recordedAt.hashCode ^
      ascent.hashCode ^
      descent.hashCode ^
      movingTimeSeconds.hashCode;
}
