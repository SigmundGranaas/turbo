import 'dart:typed_data';

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:http/http.dart' as http;

import 'package:turbo/core/util/user_agent.dart';
import '../models/mvt_layer_source.dart';

final mvtTileFetcherProvider = Provider<MvtTileFetcher>((ref) {
  return MvtTileFetcher();
});

/// Fetched tile payload — raw protobuf bytes plus the upstream ETag (if
/// any) so the repository can do conditional refreshes.
class MvtTileBytes {
  final Uint8List bytes;
  final String? etag;

  const MvtTileBytes({required this.bytes, this.etag});

  bool get isEmpty => bytes.isEmpty;
}

class MvtTileFetchException implements Exception {
  final int statusCode;
  final String url;
  final String body;

  MvtTileFetchException({
    required this.statusCode,
    required this.url,
    required this.body,
  });

  @override
  String toString() =>
      'MvtTileFetchException(status: $statusCode, url: $url, body: $body)';
}

/// Tiny HTTP fetcher: GET an MVT URL, return bytes + ETag. Auth /
/// retries / connection pooling are intentionally not handled here —
/// the dio interceptor does that for authenticated endpoints, and the
/// public `/v1/*` tiles don't require auth in V1.
class MvtTileFetcher {
  final http.Client _client;

  MvtTileFetcher({http.Client? client}) : _client = client ?? http.Client();

  Future<MvtTileBytes> fetch(
    MvtLayerSource source,
    int z,
    int x,
    int y, {
    String? ifNoneMatch,
  }) async {
    final uri = source.tileUri(z, x, y);
    final headers = <String, String>{
      'User-Agent': kTurboUserAgent,
      'Accept': 'application/vnd.mapbox-vector-tile',
      ...?source.headers,
    };
    if (ifNoneMatch != null) {
      headers['If-None-Match'] = ifNoneMatch;
    }
    final response = await _client.get(uri, headers: headers);
    if (response.statusCode == 304) {
      // Not modified — caller should reuse its cached copy.
      return MvtTileBytes(bytes: Uint8List(0));
    }
    if (response.statusCode < 200 || response.statusCode >= 300) {
      throw MvtTileFetchException(
        statusCode: response.statusCode,
        url: uri.toString(),
        body: response.body,
      );
    }
    return MvtTileBytes(
      bytes: response.bodyBytes,
      etag: response.headers['etag'],
    );
  }
}
