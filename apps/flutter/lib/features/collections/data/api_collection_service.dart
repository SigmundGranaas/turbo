import 'package:dio/dio.dart';
import 'package:turbo/core/api/api_client.dart';

import '../models/collection.dart';
import '../models/collection_item_ref.dart';
import '../models/saved_filter.dart';

/// Delta-sync result for the `?since=` flow on collections.
class CollectionDeltaResult {
  final List<CollectionWithItems> items;
  final List<CollectionTombstone> deleted;
  final DateTime serverTime;
  final String? nextCursor;

  const CollectionDeltaResult({
    required this.items,
    required this.deleted,
    required this.serverTime,
    this.nextCursor,
  });
}

/// A collection plus its current item refs, as returned by the server.
/// Wrapping the two in a pair keeps the local sync orchestrator from
/// needing two separate API round trips per row.
class CollectionWithItems {
  final Collection collection;
  final List<CollectionItemRef> items;

  const CollectionWithItems(this.collection, this.items);
}

class CollectionTombstone {
  final String uuid;
  final DateTime deletedAt;
  final int version;

  const CollectionTombstone(this.uuid, this.deletedAt, this.version);
}

class CollectionConflictException implements Exception {
  final String uuid;
  final int currentVersion;
  final CollectionWithItems? current;
  CollectionConflictException(this.uuid, this.currentVersion, this.current);

  @override
  String toString() =>
      'CollectionConflictException(uuid=$uuid, currentVersion=$currentVersion)';
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

/// HTTP client for the Collections API. Mirrors ApiTrackService/
/// ApiLocationService so the sync orchestration layer can treat all
/// three resource types uniformly.
class ApiCollectionService {
  final ApiClient _apiClient;

  ApiCollectionService(this._apiClient);

  static const String _basePath = '/api/collections/Collections';

  Map<String, dynamic> _toCreateBody(Collection c) {
    return {
      'name': c.name,
      if (c.description != null) 'description': c.description,
      if (c.colorHex != null) 'colorHex': c.colorHex,
      if (c.iconKey != null) 'iconKey': c.iconKey,
      'sortOrder': c.sortOrder,
      if (c.savedFilter != null) 'savedFilter': c.savedFilter!.toJsonString(),
    };
  }

  Map<String, dynamic> _toUpdateBody(
    Collection c, {
    bool clearSavedFilter = false,
  }) {
    return {
      'name': c.name,
      'description': c.description,
      'colorHex': c.colorHex,
      'iconKey': c.iconKey,
      'sortOrder': c.sortOrder,
      if (!clearSavedFilter && c.savedFilter != null)
        'savedFilter': c.savedFilter!.toJsonString(),
      if (clearSavedFilter) 'clearSavedFilter': true,
    };
  }

  CollectionWithItems _fromApiResponse(Map<String, dynamic> data) {
    DateTime? parseOptional(dynamic raw) {
      if (raw is String && raw.isNotEmpty) return DateTime.parse(raw);
      return null;
    }

    final c = Collection(
      uuid: data['id'] as String,
      name: data['name'] as String,
      description: data['description'] as String?,
      colorHex: data['colorHex'] as String?,
      iconKey: data['iconKey'] as String?,
      createdAt: parseOptional(data['createdAt']) ?? DateTime.now().toUtc(),
      sortOrder: (data['sortOrder'] as num?)?.toInt() ?? 0,
      savedFilter: SavedFilter.fromJsonString(data['savedFilter'] as String?),
      synced: true,
      version: (data['version'] as num?)?.toInt(),
      updatedAt: parseOptional(data['updatedAt']),
    );
    final items = (data['items'] as List? ?? const [])
        .map((i) {
          final im = i as Map<String, dynamic>;
          return CollectionItemRef(
            type: im['type'] as String,
            uuid: im['uuid'] as String,
          );
        })
        .toList();
    return CollectionWithItems(c, items);
  }

  Future<CollectionWithItems> createCollection(Collection c) async {
    final response = await _apiClient.post(
      _basePath,
      data: _toCreateBody(c),
      options: _options(),
    );

    if (response.statusCode == 201) {
      return _fromApiResponse(response.data as Map<String, dynamic>);
    } else if (response.data != null && response.data['detail'] != null) {
      throw Exception('Failed to create collection: ${response.data['detail']}');
    }
    throw Exception('Failed to create collection: Status ${response.statusCode}');
  }

  Future<CollectionWithItems?> updateCollection(
    Collection c, {
    bool clearSavedFilter = false,
  }) async {
    final headers = <String, dynamic>{};
    if (c.version != null) headers['If-Match'] = '"${c.version}"';

    final response = await _apiClient.put(
      '$_basePath/${c.uuid}',
      data: _toUpdateBody(c, clearSavedFilter: clearSavedFilter),
      options: _options(headers: headers),
    );

    if (response.statusCode == 200) {
      return _fromApiResponse(response.data as Map<String, dynamic>);
    } else if (response.statusCode == 404) {
      return null;
    } else if (response.statusCode == 412) {
      final data = response.data as Map<String, dynamic>?;
      final currentVersion = (data?['currentVersion'] as num?)?.toInt() ?? 0;
      CollectionWithItems? current;
      if (data?['current'] is Map<String, dynamic>) {
        current = _fromApiResponse(data!['current'] as Map<String, dynamic>);
      }
      throw CollectionConflictException(c.uuid, currentVersion, current);
    } else if (response.data != null && response.data['detail'] != null) {
      throw Exception('Failed to update collection: ${response.data['detail']}');
    }
    throw Exception('Failed to update collection: Status ${response.statusCode}');
  }

  Future<bool> deleteCollection(String uuid, {int? ifMatchVersion}) async {
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
      throw CollectionConflictException(uuid, currentVersion, null);
    }
    if (response.data != null && response.data['detail'] != null) {
      throw Exception('Failed to delete collection: ${response.data['detail']}');
    }
    throw Exception('Failed to delete collection: Status ${response.statusCode}');
  }

  Future<CollectionWithItems?> getCollectionById(String uuid) async {
    final response = await _apiClient.get('$_basePath/$uuid');
    if (response.statusCode == 200) {
      return _fromApiResponse(response.data as Map<String, dynamic>);
    } else if (response.statusCode == 404) {
      return null;
    }
    throw Exception('Failed to get collection by ID: Status ${response.statusCode}');
  }

  Future<bool> addItem(
    String collectionUuid,
    CollectionItemRef item, {
    int? ifMatchVersion,
  }) async {
    final headers = <String, dynamic>{};
    if (ifMatchVersion != null) headers['If-Match'] = '"$ifMatchVersion"';

    final response = await _apiClient.post(
      '$_basePath/$collectionUuid/items',
      data: {'type': item.type, 'uuid': item.uuid},
      options: _options(headers: headers),
    );
    if (response.statusCode == 204) return true;
    if (response.statusCode == 404) return false;
    if (response.statusCode == 412) {
      final data = response.data as Map<String, dynamic>?;
      final currentVersion = (data?['currentVersion'] as num?)?.toInt() ?? 0;
      CollectionWithItems? current;
      if (data?['current'] is Map<String, dynamic>) {
        current = _fromApiResponse(data!['current'] as Map<String, dynamic>);
      }
      throw CollectionConflictException(collectionUuid, currentVersion, current);
    }
    if (response.data != null && response.data['detail'] != null) {
      throw Exception('Failed to add item: ${response.data['detail']}');
    }
    throw Exception('Failed to add item: Status ${response.statusCode}');
  }

  Future<bool> removeItem(
    String collectionUuid,
    CollectionItemRef item, {
    int? ifMatchVersion,
  }) async {
    final headers = <String, dynamic>{};
    if (ifMatchVersion != null) headers['If-Match'] = '"$ifMatchVersion"';

    final response = await _apiClient.delete(
      '$_basePath/$collectionUuid/items/${item.type}/${item.uuid}',
      options: _options(headers: headers),
    );
    if (response.statusCode == 204) return true;
    if (response.statusCode == 404) return false;
    if (response.statusCode == 412) {
      final data = response.data as Map<String, dynamic>?;
      final currentVersion = (data?['currentVersion'] as num?)?.toInt() ?? 0;
      throw CollectionConflictException(collectionUuid, currentVersion, null);
    }
    throw Exception('Failed to remove item: Status ${response.statusCode}');
  }

  /// Delta-sync: returns rows changed strictly after [since] plus the
  /// tombstones the client should apply. The cursor for the next call
  /// is [CollectionDeltaResult.serverTime].
  Future<CollectionDeltaResult> getCollectionsChangedSince({
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
      throw Exception('Failed to get collections delta: Status ${response.statusCode}');
    }

    final data = response.data as Map<String, dynamic>;
    final items = (data['items'] as List? ?? const [])
        .map((i) => _fromApiResponse(i as Map<String, dynamic>))
        .toList();
    final tombstones = (data['deleted'] as List? ?? const []).map((t) {
      final tmap = t as Map<String, dynamic>;
      return CollectionTombstone(
        tmap['id'] as String,
        DateTime.parse(tmap['deletedAt'] as String),
        (tmap['version'] as num).toInt(),
      );
    }).toList();
    final serverTimeRaw = data['serverTime'] as String?;
    return CollectionDeltaResult(
      items: items,
      deleted: tombstones,
      serverTime: serverTimeRaw == null
          ? DateTime.now().toUtc()
          : DateTime.parse(serverTimeRaw),
      nextCursor: data['nextCursor'] as String?,
    );
  }
}
