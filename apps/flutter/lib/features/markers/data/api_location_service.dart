
import 'package:dio/dio.dart';
import 'package:latlong2/latlong.dart';
import 'package:turbo/core/api/api_client.dart';
import '../models/marker.dart';

/// Delta-sync result for the `?since=` flow. Carries the rows changed
/// since the supplied cursor, the tombstones learnt for that window, and
/// the `serverTime` the client should pass as the next `since=` value.
class MarkerDeltaResult {
  final List<Marker> items;
  final List<MarkerTombstone> deleted;
  final DateTime serverTime;
  final String? nextCursor;

  const MarkerDeltaResult({
    required this.items,
    required this.deleted,
    required this.serverTime,
    this.nextCursor,
  });
}

/// Server-side tombstone the delta-sync flow surfaces. The local store
/// removes the matching row when it sees this.
class MarkerTombstone {
  final String uuid;
  final DateTime deletedAt;
  final int version;

  const MarkerTombstone(this.uuid, this.deletedAt, this.version);
}

/// Raised when the server rejected a PUT/DELETE because the client's
/// `If-Match` version was stale. The caller is expected to refetch the
/// row, surface a conflict UI (or apply a server-wins merge) and retry.
class MarkerConflictException implements Exception {
  final String uuid;
  final int currentVersion;
  final Marker? current;
  MarkerConflictException(this.uuid, this.currentVersion, this.current);

  @override
  String toString() =>
      'MarkerConflictException(uuid=$uuid, currentVersion=$currentVersion)';
}

/// `Options` builder that accepts 4xx responses as successful so the
/// caller can inspect statusCode + body instead of catching DioException.
/// 5xx still throws — those are genuine transport/server failures the
/// caller probably can't handle locally.
Options _options({Map<String, dynamic>? headers}) {
  return Options(
    headers: {
      'Content-Type': 'application/json',
      if (headers != null) ...headers,
    },
    validateStatus: (status) => status != null && status < 500,
  );
}

class ApiLocationService {
  final ApiClient _apiClient;

  ApiLocationService(this._apiClient);

  Future<Marker?> createLocation(Marker marker) async {
    final requestData = {
      'geometry': {
        'longitude': marker.position.longitude,
        'latitude': marker.position.latitude,
      },
      'display': {
        'name': marker.title,
        'description': marker.description,
        'icon': marker.icon,
      },
    };
    final response = await _apiClient.post(
      '/api/geo/locations',
      data: requestData,
      options: _options(),
    );

    if (response.statusCode == 201) {
      return Marker.fromApiResponse(response.data as Map<String, dynamic>);
    } else if (response.data != null && response.data['detail'] != null) {
      throw Exception('Failed to create location: ${response.data['detail']}');
    }
    throw Exception('Failed to create location: Status ${response.statusCode}');
  }

  Future<Marker?> updateLocation(Marker marker) async {
    final requestData = {
      'geometry': {
        'longitude': marker.position.longitude,
        'latitude': marker.position.latitude,
      },
      'display': {
        'name': marker.title,
        'description': marker.description,
        'icon': marker.icon,
      },
    };
    final headers = <String, dynamic>{};
    if (marker.version != null) headers['If-Match'] = '"${marker.version}"';

    final response = await _apiClient.put(
      '/api/geo/locations/${marker.uuid}',
      data: requestData,
      options: _options(headers: headers),
    );

    if (response.statusCode == 200) {
      return Marker.fromApiResponse(response.data as Map<String, dynamic>);
    } else if (response.statusCode == 404) {
      return null;
    } else if (response.statusCode == 412) {
      final data = response.data as Map<String, dynamic>?;
      final currentVersion = (data?['currentVersion'] as num?)?.toInt() ?? 0;
      Marker? current;
      if (data?['current'] is Map<String, dynamic>) {
        current = Marker.fromApiResponse(data!['current'] as Map<String, dynamic>);
      }
      throw MarkerConflictException(marker.uuid, currentVersion, current);
    } else if (response.data != null && response.data['detail'] != null) {
      throw Exception('Failed to update location: ${response.data['detail']}');
    }
    throw Exception('Failed to update location: Status ${response.statusCode}');
  }

  Future<bool> deleteLocation(String uuid, {int? ifMatchVersion}) async {
    final headers = <String, dynamic>{};
    if (ifMatchVersion != null) headers['If-Match'] = '"$ifMatchVersion"';

    final response = await _apiClient.delete(
      '/api/geo/locations/$uuid',
      options: _options(headers: headers),
    );
    if (response.statusCode == 204) {
      return true;
    } else if (response.statusCode == 404) {
      return false;
    } else if (response.statusCode == 412) {
      final data = response.data as Map<String, dynamic>?;
      final currentVersion = (data?['currentVersion'] as num?)?.toInt() ?? 0;
      throw MarkerConflictException(uuid, currentVersion, null);
    } else if (response.data != null && response.data['detail'] != null) {
      throw Exception('Failed to delete location: ${response.data['detail']}');
    }
    throw Exception('Failed to delete location: Status ${response.statusCode}');
  }

  Future<List<Marker>> getLocationsInExtent(LatLng southwest, LatLng northeast) async {
    final queryParams = {
      'minLon': southwest.longitude.toString(),
      'minLat': southwest.latitude.toString(),
      'maxLon': northeast.longitude.toString(),
      'maxLat': northeast.latitude.toString(),
    };
    final response = await _apiClient.get('/api/geo/locations', queryParameters: queryParams);

    if (response.statusCode == 200) {
      final responseData = response.data as Map<String, dynamic>;
      final items = responseData['items'] as List<dynamic>;
      return items.map((item) => Marker.fromApiResponse(item as Map<String, dynamic>)).toList();
    } else if (response.data != null && response.data['detail'] != null) {
      throw Exception('Failed to get locations in extent: ${response.data['detail']}');
    }
    throw Exception('Failed to get locations in extent: Status ${response.statusCode}');
  }

  /// Delta-sync: returns rows changed strictly after [since] plus the
  /// tombstones the client should apply. The cursor for the next call
  /// is [MarkerDeltaResult.serverTime]; pass it next time to avoid
  /// re-pulling the rows we just received.
  ///
  /// Pass [since] = null on first sync to receive the user's entire
  /// current set in one go.
  Future<MarkerDeltaResult> getLocationsChangedSince({
    DateTime? since,
    int? limit,
  }) async {
    final queryParams = <String, String>{};
    if (since != null) queryParams['since'] = since.toUtc().toIso8601String();
    if (limit != null) queryParams['limit'] = limit.toString();

    final response = await _apiClient.get(
      '/api/geo/locations',
      queryParameters: queryParams.isEmpty ? null : queryParams,
    );

    if (response.statusCode != 200) {
      throw Exception('Failed to get locations delta: Status ${response.statusCode}');
    }

    final data = response.data as Map<String, dynamic>;
    final items = (data['items'] as List? ?? const [])
        .map((i) => Marker.fromApiResponse(i as Map<String, dynamic>))
        .toList();
    final tombstones = (data['deleted'] as List? ?? const []).map((t) {
      final tmap = t as Map<String, dynamic>;
      return MarkerTombstone(
        tmap['id'] as String,
        DateTime.parse(tmap['deletedAt'] as String),
        (tmap['version'] as num).toInt(),
      );
    }).toList();
    final serverTimeRaw = data['serverTime'] as String?;
    return MarkerDeltaResult(
      items: items,
      deleted: tombstones,
      serverTime: serverTimeRaw == null
          ? DateTime.now().toUtc()
          : DateTime.parse(serverTimeRaw),
      nextCursor: data['nextCursor'] as String?,
    );
  }

  /// Convenience wrapper: pulls the user's full current set on first
  /// sync. Implemented in terms of the delta endpoint with no cursor,
  /// not the old worldwide bbox query.
  Future<List<Marker>> getAllUserLocations() async {
    final result = await getLocationsChangedSince();
    return result.items;
  }

  Future<Marker?> getLocationById(String uuid) async {
    final response = await _apiClient.get('/api/geo/locations/$uuid');
    if (response.statusCode == 200) {
      return Marker.fromApiResponse(response.data as Map<String, dynamic>);
    } else if (response.statusCode == 404) {
      return null;
    } else if (response.data != null && response.data['detail'] != null) {
      throw Exception('Failed to get location by ID: ${response.data['detail']}');
    }
    throw Exception('Failed to get location by ID: Status ${response.statusCode}');
  }
}