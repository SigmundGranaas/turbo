@Tags(['live'])
library;

import 'package:flutter_test/flutter_test.dart';
import 'package:turbo/features/external_vector_layers/api.dart';

/// End-to-end test that hits the real Kartverket FKB Traktorveg+Sti WFS.
/// Skipped in the default run (`dart_test.yaml`); run with:
///
///     flutter test --tags=live test/features/external_vector_layers/n50_sti_live_test.dart
///
/// What this proves:
///  - The endpoint `wms.geonorge.no/skwms1/wms.traktorveg_skogsbilveger`
///    serves a real WFS (despite the `wms.` host).
///  - `TYPENAMES=ms:traktorveg_sti,ms:skogsbilveg` is accepted.
///  - A small bbox over Jotunheimen returns at least one feature with
///    geometry + the `typeveg` property the decoder reads.
void main() {
  test('FKB Traktorveg+Sti WFS returns features for a Jotunheimen bbox',
      () async {
    final source = n50StiVectorSource();
    final fetcher = VectorLayerFetcher();
    final features = await fetcher.fetchBounds(
      source,
      minLat: 61.65,
      minLon: 8.35,
      maxLat: 61.70,
      maxLon: 8.45,
      maxFeatures: 50,
    );
    expect(features, isNotEmpty,
        reason: 'Live WFS must return at least one path/tractor-road '
            'feature inside Jotunheimen. If empty, either the endpoint '
            'changed or the TYPENAMES list is wrong.');
    final f = features.first;
    expect(f.rings, isNotEmpty);
    expect(f.rings.first.length, greaterThanOrEqualTo(2));
    // The `typeveg` property is the only one we actively decode.
    final typeveg = f.properties['typeveg'];
    expect(typeveg, anyOf(equals('sti'), equals('traktorveg')));
  });
}
