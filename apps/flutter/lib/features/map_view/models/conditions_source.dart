import 'package:flutter/widgets.dart';
import 'package:latlong2/latlong.dart';

/// A source of point-in-time conditions for a map location (weather, avalanche,
/// ocean/tides, …). Each wraps an existing forecast feature behind a uniform
/// "show conditions for this point" entry, so any map entity can offer a
/// Conditions action without knowing which forecasts exist. The composition
/// seam for conditions — registered in `app/main.dart` like the other
/// registries. (Tier 4 of the cohesion pass.)
class ConditionsSource {
  final String id;
  final String label;
  final IconData icon;

  /// Open this source's detail for [point] (typically its existing sheet).
  final Future<void> Function(BuildContext context, LatLng point) show;

  const ConditionsSource({
    required this.id,
    required this.label,
    required this.icon,
    required this.show,
  });
}
