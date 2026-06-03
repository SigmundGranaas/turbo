import 'package:turbo/core/geo/geo_path.dart';

import '../models/vector_feature.dart';

/// Bridge from a tapped curated/vector trail to the shared [GeoPath], so a
/// trail can be followed/tracked through the same actions as any other path.
/// Multi-segment trails collapse to their longest ring (the dominant segment).
extension VectorFeatureGeoPath on VectorFeature {
  GeoPath? toGeoPath() {
    if (kind != VectorGeometryKind.line || rings.isEmpty) return null;
    final ring = rings.reduce((a, b) => b.length > a.length ? b : a);
    if (ring.length < 2) return null;
    return GeoPath.fromPoints(ring, source: GeoPathSource.trail);
  }
}
