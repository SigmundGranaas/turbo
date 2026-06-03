import 'package:turbo/core/geo/geo_path.dart';

import 'route_models.dart';

/// Bridge from the routing wire contract to the shared [GeoPath]. Lets other
/// features (journey, saved paths) consume a solved route without depending on
/// routing internals. A planned route's `durationS` is an *estimate*, not
/// recorded moving time, so it is deliberately not mapped to a GeoPath field.
extension RoutePlanGeoPath on RoutePlan {
  GeoPath toGeoPath() => GeoPath(
        points: geometry,
        distanceM: distanceM,
        ascentM: ascentM,
        source: GeoPathSource.route,
      );
}
