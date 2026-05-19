@Tags(['live'])
library;

import 'package:flutter_test/flutter_test.dart';
import 'package:latlong2/latlong.dart';
import 'package:turbo/features/weather/api.dart';

/// End-to-end test that hits the real MetAlerts 2.0 endpoint. Skipped in
/// normal runs; run with:
///
///     flutter test --run-skipped --tags=live test/features/weather/metalerts_service_live_test.dart
///
/// What this proves:
///  - currentAtPoint sends parameters the API accepts (HTTP 2xx). The
///    feature list may be empty (no active alerts at a given coord) — we
///    don't assert content, only that the call succeeds.
///  - currentInBounds does NOT send a `bbox` param (which MetAlerts rejects
///    with HTTP 400) — i.e. it doesn't throw YrServiceException(400).
void main() {
  test('currentAtPoint round-trips against the real MetAlerts API',
      () async {
    final service = MetAlertsService();
    final result =
        await service.currentAtPoint(const LatLng(59.9139, 10.7522));
    // Whatever's active today must parse. We can't assert alert count —
    // Norway is frequently quiet.
    expect(result, isNotNull);
    expect(result.alerts, isNotNull);
  });

  test('currentInBounds round-trips against the real MetAlerts API '
      '(would 400 if the implementation sent bbox)', () async {
    final service = MetAlertsService();
    // Bounding box covers continental Norway.
    final result =
        await service.currentInBounds(58.0, 4.0, 71.0, 31.0);
    expect(result, isNotNull);
    expect(result.alerts, isNotNull);
  });
}
