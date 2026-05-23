import 'dart:collection';

import 'package:flutter_riverpod/flutter_riverpod.dart';

import 'package:turbo/features/external_vector_layers/api.dart';
import '../models/mvt_layer_source.dart';
import 'mvt_decoder.dart';
import 'mvt_tile_fetcher.dart';

final mvtTileRepositoryProvider = Provider<MvtTileRepository>((ref) {
  return MvtTileRepository(
    fetcher: ref.watch(mvtTileFetcherProvider),
    decoder: MvtDecoder(),
  );
});

/// In-memory tile cache keyed by `source/z/x/y`. The 7-day persistent
/// store from `external_vector_layers/` is reused in M2 by writing
/// decoded features into the existing `vector_tile_cache` table; for
/// the demoable slice we keep it memory-only so the SQLite schema
/// doesn't need a migration to land first.
class MvtTileRepository {
  final MvtTileFetcher _fetcher;
  final MvtDecoder _decoder;
  final LinkedHashMap<String, List<VectorFeature>> _memory =
      LinkedHashMap<String, List<VectorFeature>>();
  static const int _memoryCap = 256;

  MvtTileRepository({
    required MvtTileFetcher fetcher,
    required MvtDecoder decoder,
  })  : _fetcher = fetcher,
        _decoder = decoder;

  /// Fetch + decode one tile. Returns the cached decoded features on
  /// subsequent calls for the same coordinate.
  Future<List<VectorFeature>> tile(
    MvtLayerSource source,
    int z,
    int x,
    int y,
  ) async {
    final key = '${source.id}/$z/$x/$y';
    final hit = _memory.remove(key);
    if (hit != null) {
      _memory[key] = hit;
      return hit;
    }
    try {
      final payload = await _fetcher.fetch(source, z, x, y);
      final features = payload.isEmpty
          ? const <VectorFeature>[]
          : _decoder.decode(bytes: payload.bytes, z: z, x: x, y: y);
      _store(key, features);
      return features;
    } on MvtTileFetchException {
      // Treat fetch failures as transient blanks — same degrade-silent
      // policy as the GeoJSON-side fetcher.
      return const <VectorFeature>[];
    }
  }

  /// Fetch every tile in a slippy-tile range concurrently, dedupe by
  /// feature id, return a flat list ready for rendering.
  Future<List<VectorFeature>> tiles(
    MvtLayerSource source, {
    required int z,
    required int minX,
    required int maxX,
    required int minY,
    required int maxY,
  }) async {
    final coords = <_TileCoord>[];
    for (var x = minX; x <= maxX; x++) {
      for (var y = minY; y <= maxY; y++) {
        coords.add(_TileCoord(z, x, y));
      }
    }
    final results = await Future.wait(
      coords.map((c) => tile(source, c.z, c.x, c.y)),
    );
    final dedup = <String, VectorFeature>{};
    for (final batch in results) {
      for (final f in batch) {
        dedup[f.id] = f;
      }
    }
    return dedup.values.toList(growable: false);
  }

  void _store(String key, List<VectorFeature> features) {
    if (_memory.length >= _memoryCap) {
      _memory.remove(_memory.keys.first);
    }
    _memory[key] = features;
  }
}

class _TileCoord {
  final int z;
  final int x;
  final int y;
  const _TileCoord(this.z, this.x, this.y);
}
