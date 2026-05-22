@Tags(['live'])
library;

import 'package:flutter_test/flutter_test.dart';
import 'package:latlong2/latlong.dart';
import 'package:turbo/features/search/api.dart';

/// End-to-end test that hits the real Kartverket `/stedsnavn/v1/punkt`
/// endpoint. Skipped in normal runs (see `dart_test.yaml`); run with:
///
///     flutter test --tags=live test/features/search/stedsnavn_backend_live_test.dart
///
/// What this proves:
///  - The query parameters the backend constructs are accepted by the live
///    service (HTTP 200, parseable JSON).
///  - A known-named coordinate (Galdhøpiggen, Norway's highest peak)
///    actually round-trips through to a non-null toponym hit. Before the
///    /punkt-shape fix this returned null because the backend's `filtrer`
///    parameter was rejected with HTTP 400.
void main() {
  test('Stedsnavn /punkt returns a toponym for Galdhøpiggen (real network)',
      () async {
    final backend = StedsnavnBackend();
    final hit = await backend.find(const LatLng(61.6362, 8.3127));
    expect(hit, isNotNull,
        reason: 'Live /punkt call must succeed with the parameters the '
            'backend ships. If null, either the API contract changed or '
            'a stray `filtrer` param is back.');
    // The summit area has dense toponym coverage; we don't pin the exact
    // string (Kartverket reorganises naming objects) but it must be a
    // non-empty, non-"Unknown" name.
    expect(hit!.description.title, isNotEmpty);
    expect(hit.description.title.toLowerCase(), isNot('unknown'));
    expect(hit.description.title.toLowerCase(), isNot('ukjent'));
  });

  test('Stedsnavn /punkt returns a toponym for Oslo central (real network)',
      () async {
    final backend = StedsnavnBackend();
    final hit = await backend.find(const LatLng(59.9139, 10.7522));
    expect(hit, isNotNull);
    expect(hit!.description.title, isNotEmpty);
  });
}
