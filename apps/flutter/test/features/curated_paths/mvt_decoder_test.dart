import 'dart:io';
import 'dart:typed_data';

import 'package:flutter_test/flutter_test.dart';
import 'package:turbo/features/curated_paths/data/mvt_decoder.dart';
import 'package:turbo/features/external_vector_layers/models/vector_feature.dart';

/// Verifies the MvtDecoder against a real tile served by the
/// Rust tileserver — fixture captured from
/// `GET /v1/hiking-trails/tiles/12/2238/1189.mvt` against the
/// seeded Sognsvann data (180 edges, 100 nodes).
///
/// If the decoder regresses (Web-Mercator math, protobuf parsing,
/// vector_tile API drift), this catches it without needing a live
/// stack — the bytes are checked in alongside the test.
void main() {
  group('MvtDecoder against a real tileserver-produced tile', () {
    late Uint8List bytes;

    setUpAll(() {
      final file = File('test/fixtures/dtm10-tile-12-2238-1189.mvt');
      expect(file.existsSync(), isTrue,
          reason:
              'fixture missing; regenerate via curl against the live tileserver');
      bytes = file.readAsBytesSync();
      expect(bytes.length, greaterThan(1000),
          reason: 'fixture should be a real tile, not an empty stub');
    });

    test('decodes into at least one LineString feature', () {
      final features =
          MvtDecoder().decode(bytes: bytes, z: 12, x: 2238, y: 1189);
      expect(features, isNotEmpty,
          reason: 'tile contains seeded hiking trails — decoder must surface them');
      final lines = features.where((f) => f.kind == VectorGeometryKind.line);
      expect(lines, isNotEmpty);
    });

    test('decoded coords land inside the tile\'s lon/lat bbox', () {
      // z=12 x=2238 y=1189 covers roughly lon 16.69..16.78°,
      // lat 59.93..60.02°. Anything outside means the Web-Mercator
      // unprojection is wrong.
      final features =
          MvtDecoder().decode(bytes: bytes, z: 12, x: 2238, y: 1189);
      const minLon = 16.5;
      const maxLon = 17.0;
      const minLat = 59.8;
      const maxLat = 60.1;
      for (final f in features) {
        for (final ring in f.rings) {
          for (final p in ring) {
            expect(p.longitude, inInclusiveRange(minLon, maxLon),
                reason: 'lon out of tile bbox — Web-Mercator math is off');
            expect(p.latitude, inInclusiveRange(minLat, maxLat),
                reason: 'lat out of tile bbox — Web-Mercator math is off');
          }
        }
      }
    });

    test('feature ids are non-empty namespaced strings', () {
      // The MVT pipeline stores id as text `edge:<n>` / `route:<uuid>`
      // so the tap-sheet can route to the right detail endpoint.
      final features =
          MvtDecoder().decode(bytes: bytes, z: 12, x: 2238, y: 1189);
      for (final f in features) {
        expect(f.id, isNotEmpty);
      }
      final hasNamespaced = features.any(
        (f) => f.id.startsWith('edge:') || f.id.startsWith('route:'),
      );
      expect(hasNamespaced, isTrue,
          reason: 'expected at least one namespaced id; the catalog'
              ' tile id format changed if this fails');
    });

    test('empty bytes return an empty feature list, not a crash', () {
      // /v1/{resource}/tiles/{z}/{x}/{y}.mvt returns an empty body
      // for tiles outside the seeded area. The decoder must treat
      // that as "no features" — not throw.
      final features = MvtDecoder().decode(
        bytes: Uint8List(0),
        z: 12,
        x: 0,
        y: 0,
      );
      expect(features, isEmpty);
    });
  });
}
