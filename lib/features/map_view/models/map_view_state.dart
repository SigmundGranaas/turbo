import 'package:latlong2/latlong.dart';

/// A class to hold the current state of the map's viewport.
/// This is immutable, ensuring state changes are predictable.
class MapViewState {
  final LatLng center;
  final double zoom;

  const MapViewState({required this.center, required this.zoom});

  /// Default initial state for the map.
  factory MapViewState.initial() => const MapViewState(
    center: LatLng(65.0, 13.0),
    zoom: 5.0,
  );

  MapViewState copyWith({
    LatLng? center,
    double? zoom,
  }) {
    return MapViewState(
      center: center ?? this.center,
      zoom: zoom ?? this.zoom,
    );
  }
}