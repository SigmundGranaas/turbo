@Tags(['live'])
library;

import 'package:flutter_test/flutter_test.dart';
import 'package:latlong2/latlong.dart';
import 'package:turbo/features/search/api.dart';

/// End-to-end test that hits the real Miljødirektoratet Vern (Naturbase)
/// ArcGIS Identify service. Skipped in normal runs (see `dart_test.yaml`);
/// run with:
///
///     flutter test --tags=live test/features/search/protected_area_backend_live_test.dart
///
/// What this proves:
///  - The MapServer index at `/arcgis/rest/services/vern/MapServer?f=json`
///    is reachable and parseable.
///  - The layer-discovery heuristic finds a polygon layer (i.e. Naturbase
///    has not removed every polygon layer or renamed them past the
///    "klasse"/"vern" filter).
///  - A coordinate well inside a known national park resolves to a
///    LocationDescription naming a protected area — proving the
///    discovered layer id is the right one to identify against.
void main() {
  test('Vern identify returns a national-park description for a coordinate '
      'inside Saltfjellet–Svartisen (real network)', () async {
    final backend = ProtectedAreaBackend();
    final d = await backend.identifyAt(const LatLng(66.6500, 14.3500));
    expect(d, isNotNull,
        reason: 'Live identify with the auto-discovered polygon scope must '
            'return a hit inside Saltfjellet–Svartisen nasjonalpark. If '
            'null, either layer discovery picked the wrong id or '
            'Naturbase changed its identify response shape.');
    expect(d!.qualifier, LocationQualifier.inArea);
    expect(d.title.toLowerCase(), contains('nasjonalpark'));
  });

  test('Vern identify returns null for an off-shore coordinate (no '
      'protection polygons cover the open North Sea)', () async {
    final backend = ProtectedAreaBackend();
    final d = await backend.identifyAt(const LatLng(58.0, 2.5));
    expect(d, isNull);
  });
}
