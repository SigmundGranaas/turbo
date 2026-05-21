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

  /// Source-specific qualifier context (e.g. "Nasjonalpark" for a
  /// protected-area hit, "2686 LOM" for an address hit). `null` for
  /// toponym / kommune hits — those use [kommune]/[fylke] instead.
  final String? secondary;

  /// Containing municipality, populated by the orchestrator when the
  /// winning description didn't already include one. Lets the UI
  /// render "Galdhøpiggen · Lom, Innlandet" without each backend
  /// having to fetch the kommune itself.
  final String? kommune;
  final String? fylke;

  /// Distance in meters from the queried coordinate to the matched
  /// feature. `null` when the description came from a containing area
  /// (kommune, park) rather than a point feature.
  final double? distanceMeters;

  /// Elevation in metres above sea level at the queried coordinate
  /// (from Kartverket Høydedata). The orchestrator enriches every
  /// returned description with this when available.
  final double? elevationMeters;

  const LocationDescription({
    required this.title,
    this.qualifier,
    this.secondary,
    this.kommune,
    this.fylke,
    this.distanceMeters,
    this.elevationMeters,
  });

  LocationDescription copyWith({
    String? title,
    LocationQualifier? qualifier,
    String? secondary,
    String? kommune,
    String? fylke,
    double? distanceMeters,
    double? elevationMeters,
  }) {
    return LocationDescription(
      title: title ?? this.title,
      qualifier: qualifier ?? this.qualifier,
      secondary: secondary ?? this.secondary,
      kommune: kommune ?? this.kommune,
      fylke: fylke ?? this.fylke,
      distanceMeters: distanceMeters ?? this.distanceMeters,
      elevationMeters: elevationMeters ?? this.elevationMeters,
    );
  }
}