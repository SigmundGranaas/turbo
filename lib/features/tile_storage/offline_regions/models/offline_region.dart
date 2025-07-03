import 'package:flutter/foundation.dart';
import 'package:flutter_map/flutter_map.dart';
import 'package:latlong2/latlong.dart';

enum DownloadStatus { enqueued, downloading, paused, completed, failed }

@immutable
class OfflineRegion {
  final String id;
  final String name;
  final LatLngBounds bounds;
  final int minZoom;
  final int maxZoom;
  final String urlTemplate;
  final String tileProviderId;
  final String tileProviderName;
  final DownloadStatus status;
  final int totalTiles;
  final int downloadedTiles;
  final DateTime createdAt;

  OfflineRegion({
    required this.id,
    required this.name,
    required this.bounds,
    required this.minZoom,
    required this.maxZoom,
    required this.urlTemplate,
    required this.tileProviderId,
    required this.tileProviderName,
    this.status = DownloadStatus.enqueued,
    this.totalTiles = 0,
    this.downloadedTiles = 0,
    DateTime? createdAt,
  }) : createdAt = createdAt ?? DateTime.now();

  double get progress =>
      totalTiles > 0 ? (downloadedTiles) / totalTiles : 0.0;

  OfflineRegion copyWith({
    String? id,
    String? name,
    LatLngBounds? bounds,
    int? minZoom,
    int? maxZoom,
    String? urlTemplate,
    String? tileProviderId,
    String? tileProviderName,
    DownloadStatus? status,
    int? totalTiles,
    int? downloadedTiles,
    DateTime? createdAt,
  }) {
    return OfflineRegion(
      id: id ?? this.id,
      name: name ?? this.name,
      bounds: bounds ?? this.bounds,
      minZoom: minZoom ?? this.minZoom,
      maxZoom: maxZoom ?? this.maxZoom,
      urlTemplate: urlTemplate ?? this.urlTemplate,
      tileProviderId: tileProviderId ?? this.tileProviderId,
      tileProviderName: tileProviderName ?? this.tileProviderName,
      status: status ?? this.status,
      totalTiles: totalTiles ?? this.totalTiles,
      downloadedTiles: downloadedTiles ?? this.downloadedTiles,
      createdAt: createdAt ?? this.createdAt,
    );
  }

  Map<String, dynamic> toMap() {
    return {
      'id': id,
      'name': name,
      'minLat': bounds.southWest.latitude,
      'minLng': bounds.southWest.longitude,
      'maxLat': bounds.northEast.latitude,
      'maxLng': bounds.northEast.longitude,
      'minZoom': minZoom,
      'maxZoom': maxZoom,
      'urlTemplate': urlTemplate,
      'tileProviderId': tileProviderId,
      'tileProviderName': tileProviderName,
      'status': status.index,
      'totalTiles': totalTiles,
      'downloadedTiles': downloadedTiles,
      'createdAt': createdAt.toIso8601String(),
    };
  }

  factory OfflineRegion.fromMap(Map<String, dynamic> map) {
    return OfflineRegion(
      id: map['id'] as String,
      name: map['name'] as String,
      bounds: LatLngBounds(
        LatLng(map['minLat'] as double, map['minLng'] as double),
        LatLng(map['maxLat'] as double, map['maxLng'] as double),
      ),
      minZoom: map['minZoom'] as int,
      maxZoom: map['maxZoom'] as int,
      urlTemplate: map['urlTemplate'] as String,
      tileProviderId: map['tileProviderId'] as String,
      tileProviderName: map['tileProviderName'] as String,
      status: DownloadStatus.values[map['status'] as int],
      totalTiles: map['totalTiles'] as int,
      downloadedTiles: map['downloadedTiles'] as int,
      createdAt: DateTime.parse(map['createdAt'] as String),
    );
  }
}