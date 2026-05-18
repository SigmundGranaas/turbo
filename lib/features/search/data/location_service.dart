import 'package:latlong2/latlong.dart';

abstract class LocationService{
  Future<List<LocationSearchResult>> findLocationsBy(String name);
}

class LocationSearchResult {
  final String title;
  final String? description;
  final LatLng position;
  final String? icon;
  final String? source;
  final Map<String, dynamic>? metadata;

  LocationSearchResult({
    required this.title,
    this.description,
    required this.position,
    this.icon,
    this.source,
    this.metadata,
  });

  @override
  String toString() {
    return 'LocationSearchResult(title: $title, description: $description, position: $position, icon: $icon, source: $source)';
  }
}

/// Spatial qualifier for a [LocationDescription]: how a UI should describe
/// the relationship between a tapped coordinate and the nearby named place.
///
/// The UI is responsible for turning the qualifier into a localized word
/// ("On", "Close to", "In", …).
enum LocationQualifier { on, closeTo, atPlace, inArea, near }

/// Lenient reverse-geocode result. Unlike [LocationSearchResult], this is
/// always populated with *something* — a nearby peak / settlement, a
/// containing protected area, the kommune at the point, or the raw
/// coordinates as last resort — so the UI never has to fall back to
/// "Unknown location".
class LocationDescription {
  final String title;
  final LocationQualifier? qualifier;

  /// Optional secondary context (kommune + fylke, "350 m away", etc.).
  final String? secondary;

  /// Distance in meters from the queried coordinate to the matched
  /// feature. `null` when the description came from a containing area
  /// (kommune, park) rather than a point feature.
  final double? distanceMeters;

  const LocationDescription({
    required this.title,
    this.qualifier,
    this.secondary,
    this.distanceMeters,
  });
}