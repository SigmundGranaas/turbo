import 'package:turbo/core/geo/geo_path.dart';

import '../models/activity_geometry.dart';

/// Bridge from a route-shaped activity's geometry to the shared [GeoPath], so
/// a recorded/planned activity route can be followed or re-tracked. Returns
/// null for point/polygon activities (nothing to follow as a line).
extension ActivityGeometryGeoPath on ActivityGeometry {
  GeoPath? toGeoPath() {
    if (kind != ActivityGeometryKind.lineString) return null;
    final pts = coordinates;
    if (pts.length < 2) return null;
    return GeoPath.fromPoints(pts, source: GeoPathSource.activity);
  }
}
