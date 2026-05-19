@Tags(['live'])
library;

import 'package:flutter_test/flutter_test.dart';
import 'package:http/http.dart' as http;
import 'package:turbo/features/external_vector_layers/api.dart';

/// End-to-end test that hits the real Geonorge endpoints. Skipped in normal
/// runs because most CI / sandboxed environments block outbound network.
///
/// Run with: `flutter test --run-skipped --tags=live`
///
/// What this proves:
///  - The WFS base URL, query parameters, and TYPENAMES we ship resolve to
///    a real, currently-published service.
///  - Each trail subtype returns at least one feature for a bbox that's
///    known to contain features of that subtype, and the GML → GeoJSON
///    converter produces a VectorFeature with sensible Norway-bounded
///    coordinates.
///
/// If this test fails after a normal install, the trail layer URLs are
/// wrong — fix the source in `nasjonal_turbase_source.dart` and update
/// this test's expectations.
void main() {
  // Per-subtype "known to have features" bboxes. Jotunheimen densely
  // covers hiking + ski trails but has no bike or "other" routes;
  // southeast Norway (Oslo region) has all four. Each bbox is wide
  // enough to absorb minor data churn.
  const Map<TrailSubtype, _Bbox> knownGood = {
    TrailSubtype.foot: _Bbox(61.5, 8.1, 61.8, 8.6),
    TrailSubtype.ski: _Bbox(61.5, 8.1, 61.8, 8.6),
    TrailSubtype.bike: _Bbox(59.5, 10.0, 60.5, 11.5),
    TrailSubtype.other: _Bbox(59.5, 10.0, 60.5, 11.5),
  };

  late VectorLayerFetcher fetcher;

  setUpAll(() {
    fetcher = VectorLayerFetcher(client: http.Client());
  });

  group('Nasjonal turbase (live network)', () {
    for (final entry in knownGood.entries) {
      final subtype = entry.key;
      final bbox = entry.value;
      test('$subtype: GML → GeoJSON → VectorFeature round-trip against the '
          'real WFS', () async {
        final source = trailVectorSource(subtype);
        final features = await fetcher.fetchBounds(
          source,
          minLat: bbox.minLat,
          minLon: bbox.minLon,
          maxLat: bbox.maxLat,
          maxLon: bbox.maxLon,
          maxFeatures: 50,
        );
        expect(features, isNotEmpty,
            reason:
                'Subtype $subtype returned no features at $bbox — either '
                'the TYPENAMES, the BBOX axis order, the SRSNAME, or the '
                'GML converter is wrong.');
        final first = features.first;
        // Trails are lines.
        expect(first.kind, VectorGeometryKind.line);
        expect(first.rings, isNotEmpty);
        expect(first.rings.first.length, greaterThan(1));
        // Every vertex must land inside Norway's bounding box. A failure
        // here means the axis swap is broken end-to-end (the unit test
        // covers it on captured payloads; this covers the live path).
        for (final ring in first.rings) {
          for (final p in ring) {
            expect(p.latitude, inInclusiveRange(57.0, 72.0),
                reason: '$subtype vertex lat=${p.latitude} outside Norway');
            expect(p.longitude, inInclusiveRange(4.0, 32.0),
                reason: '$subtype vertex lon=${p.longitude} outside Norway');
          }
        }
      }, timeout: const Timeout(Duration(seconds: 30)));
    }
  });
}

class _Bbox {
  final double minLat;
  final double minLon;
  final double maxLat;
  final double maxLon;
  const _Bbox(this.minLat, this.minLon, this.maxLat, this.maxLon);
  @override
  String toString() => '($minLat,$minLon → $maxLat,$maxLon)';
}
