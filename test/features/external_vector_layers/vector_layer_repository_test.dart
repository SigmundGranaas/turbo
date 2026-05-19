import 'package:flutter_test/flutter_test.dart';
import 'package:latlong2/latlong.dart';
import 'package:turbo/features/external_vector_layers/api.dart';

class _RecordingStore implements VectorTileStore {
  final Map<String, StoredVectorTile> entries = {};
  int reads = 0;
  int writes = 0;

  String _k(String source, int z, int x, int y) => '$source/$z/$x/$y';

  @override
  Future<StoredVectorTile?> read(String source, int z, int x, int y) async {
    reads++;
    return entries[_k(source, z, x, y)];
  }

  @override
  Future<void> write(String source, int z, int x, int y, String geojson,
      DateTime fetchedAt) async {
    writes++;
    entries[_k(source, z, x, y)] = StoredVectorTile(
      geojson: geojson,
      fetchedAt: fetchedAt,
    );
  }
}

class _StubFetcher extends VectorLayerFetcher {
  int calls = 0;
  List<VectorFeature> next;
  Object? error;
  _StubFetcher(this.next);

  @override
  Future<List<VectorFeature>> fetchBounds(
    VectorLayerSource source, {
    required double minLat,
    required double minLon,
    required double maxLat,
    required double maxLon,
    int maxFeatures = 200,
  }) async {
    calls++;
    if (error != null) throw error!;
    return next;
  }
}

VectorLayerSource _source({bool persist = true}) => VectorLayerSource(
      id: 's',
      name: (_) => 's',
      persist: persist,
      buildUri: ({
        required minLat,
        required minLon,
        required maxLat,
        required maxLon,
        maxFeatures,
      }) =>
          Uri.parse('https://example.com/wfs'),
    );

VectorFeature _line(String id) => VectorFeature(
      id: id,
      kind: VectorGeometryKind.line,
      rings: [
        [const LatLng(60, 5), const LatLng(60.001, 5.001)]
      ],
      properties: {'navn': id},
    );

void main() {
  group('VectorLayerRepository.featuresInBounds', () {
    test('hits network once per uncached tile and caches in memory', () async {
      final store = _RecordingStore();
      final fetcher = _StubFetcher([_line('a'), _line('b')]);
      final repo = VectorLayerRepository(fetcher: fetcher, store: store);

      // Small bbox — single tile at grid zoom 12.
      final out = await repo.featuresInBounds(
        _source(),
        60.39,
        5.32,
        60.40,
        5.33,
      );
      expect(out, hasLength(2));
      expect(fetcher.calls, 1);
      expect(store.writes, 1);

      // Second call inside the same tile must NOT hit the network.
      await repo.featuresInBounds(
        _source(),
        60.395,
        5.325,
        60.396,
        5.326,
      );
      expect(fetcher.calls, 1, reason: 'memory cache should answer');
    });

    test('non-persistent sources skip the disk write', () async {
      final store = _RecordingStore();
      final fetcher = _StubFetcher([_line('a')]);
      final repo = VectorLayerRepository(fetcher: fetcher, store: store);
      await repo.featuresInBounds(
        _source(persist: false),
        60.39,
        5.32,
        60.40,
        5.33,
      );
      expect(store.writes, 0);
    });

    test('network errors degrade silently and return whatever was cached',
        () async {
      final store = _RecordingStore();
      final fetcher = _StubFetcher(const [])..error = Exception('boom');
      final repo = VectorLayerRepository(fetcher: fetcher, store: store);
      final out = await repo.featuresInBounds(
        _source(),
        60.39,
        5.32,
        60.40,
        5.33,
      );
      expect(out, isEmpty);
    });

    test('deduplicates features across overlapping tiles', () async {
      final fetcher = _StubFetcher([_line('shared')]);
      final repo = VectorLayerRepository(
        fetcher: fetcher,
        store: _RecordingStore(),
      );
      // Span multiple tiles by widening the bbox.
      final out = await repo.featuresInBounds(_source(), 60.0, 5.0, 60.5, 5.5);
      expect(out.where((f) => f.id == 'shared'), hasLength(1));
    });
  });
}
