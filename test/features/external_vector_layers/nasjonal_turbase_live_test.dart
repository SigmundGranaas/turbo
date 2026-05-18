@Tags(['live'])
library;

import 'package:flutter_test/flutter_test.dart';
import 'package:http/http.dart' as http;
import 'package:turbo/features/external_vector_layers/api.dart';

/// End-to-end test that hits the real Geonorge endpoints. Skipped in normal
/// runs because most CI / sandboxed environments block outbound network.
///
/// Run with: `flutter test --tags=live`
///
/// What this proves:
///  - The WFS base URL, query parameters, and TYPENAMES we ship resolve to
///    a real, currently-published service.
///  - Each trail subtype returns at least one parseable GeoJSON feature for
///    a known-populated bbox (around Galdhøpiggen / Jotunheimen, which has
///    the densest trail coverage in Norway).
///
/// If this test fails after a normal install, the trail layer URLs are
/// wrong — fix the source in `nasjonal_turbase_source.dart` and update this
/// test's expectations.
void main() {
  // Jotunheimen — every trail subtype has features here year-round.
  const minLat = 61.5;
  const minLon = 8.1;
  const maxLat = 61.8;
  const maxLon = 8.6;

  late VectorLayerFetcher fetcher;

  setUpAll(() {
    fetcher = VectorLayerFetcher(client: http.Client());
  });

  group('Nasjonal turbase (live network)', () {
    for (final subtype in TrailSubtype.values) {
      test('$subtype returns at least one feature for Jotunheimen', () async {
        final source = trailVectorSource(subtype);
        final features = await fetcher.fetchBounds(
          source,
          minLat: minLat,
          minLon: minLon,
          maxLat: maxLat,
          maxLon: maxLon,
          maxFeatures: 50,
        );
        expect(features, isNotEmpty,
            reason:
                'Subtype $subtype returned no features at Jotunheimen — '
                'either the TYPENAMES, the BBOX axis order, or the SRSNAME '
                'is wrong in nasjonal_turbase_source.dart.');
        // Trails are lines.
        expect(features.first.kind, VectorGeometryKind.line);
        expect(features.first.rings, isNotEmpty);
        expect(features.first.rings.first.length, greaterThan(1));
      }, timeout: const Timeout(Duration(seconds: 30)));
    }
  });
}
