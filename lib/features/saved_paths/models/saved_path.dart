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

    return SavedPath(
      uuid: map['uuid'] as String,
      title: map['title'] as String,
      description: map['description'] as String?,
      points: points,
      distance: map['distance'] as double,
      createdAt: DateTime.parse(map['created_at'] as String),
      colorHex: map['color_hex'] as String?,
      iconKey: map['icon_key'] as String?,
      smoothing: map['smoothing'] == 1 || map['smoothing'] == true,
      lineStyleKey: map['line_style'] as String?,
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
          lineStyleKey == other.lineStyleKey;

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
      lineStyleKey.hashCode;
}
