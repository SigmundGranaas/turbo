import 'package:dio/dio.dart';
import 'package:latlong2/latlong.dart';
import 'package:turbo/core/api/api_client.dart';
import '../models/saved_path.dart';

/// Delta-sync result for the `?since=` flow on tracks. Mirrors the
/// markers' `MarkerDeltaResult` so the sync orchestration layer is
/// uniform across resource types.
class TrackDeltaResult {
  final List<SavedPath> items;
  final List<TrackTombstone> deleted;
  final DateTime serverTime;
  final String? nextCursor;

  const TrackDeltaResult({
    required this.items,
    required this.deleted,
    required this.serverTime,
    this.nextCursor,
  });
}

class TrackTombstone {
  final String uuid;
  final DateTime deletedAt;
  final int version;

  const TrackTombstone(this.uuid, this.deletedAt, this.version);
}

class TrackConflictException implements Exception {
  final String uuid;
  final int currentVersion;
  final SavedPath? current;
  TrackConflictException(this.uuid, this.currentVersion, this.current);

  @override
  String toString() =>
      'TrackConflictException(uuid=$uuid, currentVersion=$currentVersion)';
}

Options _options({Map<String, dynamic>? headers}) {
  return Options(
    headers: {
      'Content-Type': 'application/json',
      if (headers != null) ...headers,
    },
    validateStatus: (status) => status != null && status < 500,
  );
}

/// HTTP client for the Tracks API. Mirrors the shape of
/// [ApiLocationService] so the sync orchestration layer can treat the
/// two resource types uniformly.
class ApiTrackService {
  final ApiClient _apiClient;

  ApiTrackService(this._apiClient);

  static const String _basePath = '/api/tracks/Tracks';

  Map<String, dynamic> _toRequestBody(SavedPath path) {
    return {
      'geometry': {
        'points': path.points
            .map((p) => {'longitude': p.longitude, 'latitude': p.latitude})
            .toList(),
        if (path.elevations != null) 'elevations': path.elevations,
      },
      'metadata': {
        'name': path.title,
        if (path.description != null) 'description': path.description,
        if (path.colorHex != null) 'colorHex': path.colorHex,
        if (path.iconKey != null) 'iconKey': path.iconKey,
        if (path.lineStyleKey != null) 'lineStyleKey': path.lineStyleKey,
        'smoothing': path.smoothing,
      },
      'stats': {
        'distanceMeters': path.distance,
        if (path.ascent != null) 'ascentMeters': path.ascent,
        if (path.descent != null) 'descentMeters': path.descent,
        if (path.movingTimeSeconds != null)
          'movingTimeSeconds': path.movingTimeSeconds,
        if (path.recordedAt != null)
          'recordedAt': path.recordedAt!.toUtc().toIso8601String(),
      },
    };
  }

  SavedPath _fromApiResponse(Map<String, dynamic> data) {
    final geometry = data['geometry'] as Map<String, dynamic>;
    final metadata = data['metadata'] as Map<String, dynamic>;
    final stats = data['stats'] as Map<String, dynamic>?;

    final pts = (geometry['points'] as List? ?? const [])
        .map((p) {
          final pm = p as Map<String, dynamic>;
          return LatLng(
            (pm['latitude'] as num).toDouble(),
            (pm['longitude'] as num).toDouble(),
          );
        })
        .toList();
    final elevs = (geometry['elevations'] as List?)
        ?.map((e) => (e as num).toDouble())
        .toList();

    DateTime? parseOptional(dynamic raw) {
      if (raw is String && raw.isNotEmpty) return DateTime.parse(raw);
      return null;
    }

    return SavedPath(
      uuid: data['id'] as String,
      title: metadata['name'] as String,
      description: metadata['description'] as String?,
      points: pts,
      distance: stats == null ? 0 : (stats['distanceMeters'] as num).toDouble(),
      colorHex: metadata['colorHex'] as String?,
      iconKey: metadata['iconKey'] as String?,
      lineStyleKey: metadata['lineStyleKey'] as String?,
      smoothing: (metadata['smoothing'] as bool?) ?? false,
      elevations: elevs,
      recordedAt: parseOptional(stats?['recordedAt']),
      ascent: (stats?['ascentMeters'] as num?)?.toDouble(),
      descent: (stats?['descentMeters'] as num?)?.toDouble(),
      movingTimeSeconds: (stats?['movingTimeSeconds'] as num?)?.toInt(),
      version: (data['version'] as num?)?.toInt(),
      updatedAt: parseOptional(data['updatedAt']),
      deletedAt: parseOptional(data['deletedAt']),
      synced: true,
    );
  }

  Future<SavedPath> createTrack(SavedPath path) async {
    final response = await _apiClient.post(
      _basePath,
      data: _toRequestBody(path),
      options: _options(),
    );

    if (response.statusCode == 201) {
      final data = response.data as Map<String, dynamic>;
      // The create response echoes the request shape; reconcile with the
      // local uuid by trusting the server's id (which is what the read
      // model will project).
      return _fromApiResponse(data).copyWith(uuid: data['id'] as String);
    } else if (response.data != null && response.data['detail'] != null) {
      throw Exception('Failed to create track: ${response.data['detail']}');
    }
    throw Exception('Failed to create track: Status ${response.statusCode}');
  }

  Future<SavedPath?> updateTrack(SavedPath path) async {
    final headers = <String, dynamic>{};
    if (path.version != null) headers['If-Match'] = '"${path.version}"';

    final response = await _apiClient.put(
      '$_basePath/${path.uuid}',
      data: _toRequestBody(path),
      options: _options(headers: headers),
    );

    if (response.statusCode == 200) {
      return _fromApiResponse(response.data as Map<String, dynamic>);
    } else if (response.statusCode == 404) {
      return null;
    } else if (response.statusCode == 412) {
      final data = response.data as Map<String, dynamic>?;
      final currentVersion = (data?['currentVersion'] as num?)?.toInt() ?? 0;
      SavedPath? current;
      if (data?['current'] is Map<String, dynamic>) {
        current = _fromApiResponse(data!['current'] as Map<String, dynamic>);
      }
      throw TrackConflictException(path.uuid, currentVersion, current);
    } else if (response.data != null && response.data['detail'] != null) {
      throw Exception('Failed to update track: ${response.data['detail']}');
    }
    throw Exception('Failed to update track: Status ${response.statusCode}');
  }

  Future<bool> deleteTrack(String uuid, {int? ifMatchVersion}) async {
    final headers = <String, dynamic>{};
    if (ifMatchVersion != null) headers['If-Match'] = '"$ifMatchVersion"';

    final response = await _apiClient.delete(
      '$_basePath/$uuid',
      options: _options(headers: headers),
    );
    if (response.statusCode == 204) return true;
    if (response.statusCode == 404) return false;
    if (response.statusCode == 412) {
      final data = response.data as Map<String, dynamic>?;
      final currentVersion = (data?['currentVersion'] as num?)?.toInt() ?? 0;
      throw TrackConflictException(uuid, currentVersion, null);
    }
    if (response.data != null && response.data['detail'] != null) {
      throw Exception('Failed to delete track: ${response.data['detail']}');
    }
    throw Exception('Failed to delete track: Status ${response.statusCode}');
  }

  Future<SavedPath?> getTrackById(String uuid) async {
    final response = await _apiClient.get('$_basePath/$uuid');
    if (response.statusCode == 200) {
      return _fromApiResponse(response.data as Map<String, dynamic>);
    } else if (response.statusCode == 404) {
      return null;
    }
    throw Exception('Failed to get track by ID: Status ${response.statusCode}');
  }

  /// Delta-sync: returns rows changed strictly after [since] plus the
  /// tombstones the client should apply. The cursor for the next call
  /// is [TrackDeltaResult.serverTime]; pass it next time to avoid
  /// re-pulling the rows we just received.
  Future<TrackDeltaResult> getTracksChangedSince({
    DateTime? since,
    int? limit,
  }) async {
    final queryParams = <String, String>{};
    if (since != null) queryParams['since'] = since.toUtc().toIso8601String();
    if (limit != null) queryParams['limit'] = limit.toString();

    final response = await _apiClient.get(
      _basePath,
      queryParameters: queryParams.isEmpty ? null : queryParams,
    );

    if (response.statusCode != 200) {
      throw Exception('Failed to get tracks delta: Status ${response.statusCode}');
    }

    final data = response.data as Map<String, dynamic>;
    final items = (data['items'] as List? ?? const [])
        .map((i) => _fromApiResponse(i as Map<String, dynamic>))
        .toList();
    final tombstones = (data['deleted'] as List? ?? const []).map((t) {
      final tmap = t as Map<String, dynamic>;
      return TrackTombstone(
        tmap['id'] as String,
        DateTime.parse(tmap['deletedAt'] as String),
        (tmap['version'] as num).toInt(),
      );
    }).toList();
    final serverTimeRaw = data['serverTime'] as String?;
    return TrackDeltaResult(
      items: items,
      deleted: tombstones,
      serverTime: serverTimeRaw == null
          ? DateTime.now().toUtc()
          : DateTime.parse(serverTimeRaw),
      nextCursor: data['nextCursor'] as String?,
    );
  }
}
