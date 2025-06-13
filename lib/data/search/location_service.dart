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