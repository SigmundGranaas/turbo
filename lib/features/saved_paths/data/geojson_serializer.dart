import 'dart:convert';

import '../models/saved_path.dart';

/// Serializes a [SavedPath] to GeoJSON (RFC 7946) format.
String savedPathToGeoJson(SavedPath path) {
  final properties = <String, dynamic>{
    'title': path.title,
    'distance': path.distance,
    'createdAt': path.createdAt.toUtc().toIso8601String(),
  };

  if (path.description != null && path.description!.isNotEmpty) {
    properties['description'] = path.description;
  }

  final featureCollection = {
    'type': 'FeatureCollection',
    'features': [
      {
        'type': 'Feature',
        'geometry': {
          'type': 'LineString',
          // RFC 7946: coordinates are [longitude, latitude]
          'coordinates': path.points
              .map((p) => [p.longitude, p.latitude])
              .toList(),
        },
        'properties': properties,
      },
    ],
  };

  return const JsonEncoder.withIndent('  ').convert(featureCollection);
}
