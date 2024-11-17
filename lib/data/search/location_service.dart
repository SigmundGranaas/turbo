import 'package:latlong2/latlong.dart';

abstract class LocationService{
  Future<List<LocationSearchResult>> findLocationsBy(String name);
}

class LocationSearchResult{
  final String title;
  final LatLng position;
  final String? description;
  final String? icon;

  LocationSearchResult({required this.title, required this.description, required this.position, required this.icon});
}