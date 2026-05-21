import 'dart:convert';

import '../models/marker.dart';

/// Serializes a [Marker] to GeoJSON (RFC 7946) format.
String markerToGeoJson(Marker marker) => markersToGeoJson([marker]);

/// Serializes a list of [Marker]s as a single GeoJSON FeatureCollection.
/// Used for bulk export from the selection bar.
String markersToGeoJson(List<Marker> markers) {
  final featureCollection = {
    'type': 'FeatureCollection',
    'features': markers.map(_markerFeature).toList(),
  };
  return const JsonEncoder.withIndent('  ').convert(featureCollection);
}

Map<String, dynamic> _markerFeature(Marker marker) {
  final properties = <String, dynamic>{'title': marker.title};
  if (marker.description != null && marker.description!.isNotEmpty) {
    properties['description'] = marker.description;
  }
  if (marker.icon != null && marker.icon!.isNotEmpty) {
    properties['icon'] = marker.icon;
  }
  return {
    'type': 'Feature',
    'geometry': {
      'type': 'Point',
      // RFC 7946: coordinates are [longitude, latitude]
      'coordinates': [marker.position.longitude, marker.position.latitude],
    },
    'properties': properties,
  };
}
