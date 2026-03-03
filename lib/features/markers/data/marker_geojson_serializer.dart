import 'dart:convert';

import '../models/marker.dart';

/// Serializes a [Marker] to GeoJSON (RFC 7946) format.
String markerToGeoJson(Marker marker) {
  final properties = <String, dynamic>{
    'title': marker.title,
  };

  if (marker.description != null && marker.description!.isNotEmpty) {
    properties['description'] = marker.description;
  }

  if (marker.icon != null && marker.icon!.isNotEmpty) {
    properties['icon'] = marker.icon;
  }

  final featureCollection = {
    'type': 'FeatureCollection',
    'features': [
      {
        'type': 'Feature',
        'geometry': {
          'type': 'Point',
          // RFC 7946: coordinates are [longitude, latitude]
          'coordinates': [
            marker.position.longitude,
            marker.position.latitude,
          ],
        },
        'properties': properties,
      },
    ],
  };

  return const JsonEncoder.withIndent('  ').convert(featureCollection);
}
