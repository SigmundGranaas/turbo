@Tags(['live'])
library;

import 'package:flutter_test/flutter_test.dart';
import 'package:latlong2/latlong.dart';
import 'package:turbo/features/saved_paths/api.dart';

/// End-to-end test that hits the real Kartverket Høydedata `/punkt` batch
/// endpoint. Skipped in normal runs; run with:
///
///     flutter test --run-skipped --tags=live test/features/saved_paths/hoydedata_service_live_test.dart
///
/// What this proves:
///  - The `punkter` JSON-array encoding the service ships with is accepted
///    by the live API (HTTP 200, parseable JSON). Before the encoding fix
///    every batch failed with HTTP 422 ('har ikke gyldig struktur. Det
///    forventes en liste med lister med koordinater').
///  - Known-elevation reference points return sane values.
void main() {
  test('elevationsFor returns realistic elevations for a known batch',
      () async {
    final service = HoydedataService();
    final result = await service.elevationsFor(const [
      LatLng(59.9139, 10.7522), // Oslo: near sea level
      LatLng(60.39299, 5.32415), // Bergen waterfront: near sea level
      LatLng(61.6362, 8.3127), // Galdhøpiggen: 2469 m
    ]);
    expect(result, hasLength(3));
    expect(result[0], isNotNull, reason: 'Oslo elevation must resolve');
    expect(result[0]!, lessThan(50.0));
    expect(result[1], isNotNull, reason: 'Bergen elevation must resolve');
    expect(result[1]!, lessThan(50.0));
    expect(result[2], isNotNull, reason: 'Galdhøpiggen elevation must resolve');
    expect(result[2]!, greaterThan(2400.0));
    expect(result[2]!, lessThan(2500.0));
  });
}
