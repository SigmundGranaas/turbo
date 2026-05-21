import 'package:flutter_test/flutter_test.dart';
import 'package:turbo/core/util/slippy_tiles.dart';

void main() {
  group('tilesCovering', () {
    test('returns a single tile when the bbox sits inside one tile', () {
      // Oslo (59.9, 10.7) at z=12 — small enough delta to stay in one tile.
      final tiles = tilesCovering(
        zoom: 12,
        minLat: 59.91,
        minLon: 10.75,
        maxLat: 59.92,
        maxLon: 10.76,
      );
      expect(tiles, hasLength(1));
    });

    test('returns the union when the bbox straddles a tile boundary', () {
      final tiles = tilesCovering(
        zoom: 12,
        minLat: 59.5,
        minLon: 10.0,
        maxLat: 60.5,
        maxLon: 11.0,
      );
      expect(tiles.length, greaterThan(1));
      // All tiles are at the requested zoom.
      expect(tiles.every((t) => t.z == 12), isTrue);
    });

    test('tile bounds round-trip back to the bbox they cover', () {
      // Pick a known tile, ask for tiles covering its centre, expect that tile.
      const tile = SlippyTile(10, 549, 297); // somewhere around southern Norway
      final b = tile.bounds;
      final midLat = (b.north + b.south) / 2;
      final midLon = (b.east + b.west) / 2;
      final tiles = tilesCovering(
        zoom: 10,
        minLat: midLat - 0.001,
        minLon: midLon - 0.001,
        maxLat: midLat + 0.001,
        maxLon: midLon + 0.001,
      );
      expect(tiles, contains(tile));
    });

    test('clamps off-world indices', () {
      // Asking for the whole globe at z=1 returns at most 4 tiles
      // (2^1 × 2^1), never anything with x or y outside [0, 1].
      final tiles = tilesCovering(
        zoom: 1,
        minLat: -85,
        minLon: -180,
        maxLat: 85,
        maxLon: 180,
      );
      expect(tiles.length, lessThanOrEqualTo(4));
      for (final t in tiles) {
        expect(t.x, inInclusiveRange(0, 1));
        expect(t.y, inInclusiveRange(0, 1));
      }
    });
  });
}
