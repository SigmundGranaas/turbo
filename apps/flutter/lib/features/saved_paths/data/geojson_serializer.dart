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

  final recordedAt = path.recordedAt;
  if (recordedAt != null) {
    properties['recordedAt'] = recordedAt.toUtc().toIso8601String();
  }
  if (path.ascent != null) properties['ascent'] = path.ascent;
  if (path.descent != null) properties['descent'] = path.descent;
  if (path.movingTimeSeconds != null) {
    properties['movingTimeSeconds'] = path.movingTimeSeconds;
  }

  final elevations = path.elevations;
  final hasElevations = elevations != null && elevations.length == path.points.length;

  final coordinates = <List<double>>[];
  for (var i = 0; i < path.points.length; i++) {
    final p = path.points[i];
    if (hasElevations) {
      coordinates.add([p.longitude, p.latitude, elevations[i]]);
    } else {
      coordinates.add([p.longitude, p.latitude]);
    }
  }

  final featureCollection = {
    'type': 'FeatureCollection',
    'features': [
      {
        'type': 'Feature',
        'geometry': {
          'type': 'LineString',
          // RFC 7946: coordinates are [longitude, latitude] (+ optional altitude).
          'coordinates': coordinates,
        },
        'properties': properties,
      },
    ],
  };

  return const JsonEncoder.withIndent('  ').convert(featureCollection);
}
