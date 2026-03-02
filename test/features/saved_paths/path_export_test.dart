import 'dart:convert';

import 'package:flutter_test/flutter_test.dart';
import 'package:latlong2/latlong.dart';
import 'package:turbo/features/saved_paths/data/gpx_serializer.dart';
import 'package:turbo/features/saved_paths/data/geojson_serializer.dart';
import 'package:turbo/features/saved_paths/data/path_export_service.dart';
import 'package:turbo/features/saved_paths/models/saved_path.dart';

SavedPath _makePath({
  String title = 'Test Path',
  String? description,
  List<LatLng>? points,
  double distance = 1234.5,
  DateTime? createdAt,
}) =>
    SavedPath(
      uuid: 'test-uuid',
      title: title,
      description: description,
      points: points ?? [const LatLng(59.9, 10.7), const LatLng(60.0, 10.8)],
      distance: distance,
      createdAt: createdAt ?? DateTime.utc(2026, 3, 1, 12, 0),
    );

void main() {
  group('GPX serializer', () {
    test('produces valid GPX structure with metadata and track', () {
      final path = _makePath(
        title: 'Besseggen Ridge',
        description: 'Summer hike',
      );
      final gpx = savedPathToGpx(path);

      expect(gpx, contains('<?xml version="1.0" encoding="UTF-8"?>'));
      expect(gpx, contains('<gpx version="1.1"'));
      expect(gpx, contains('<metadata>'));
      expect(gpx, contains('<name>Besseggen Ridge</name>'));
      expect(gpx, contains('<desc>Summer hike</desc>'));
      expect(gpx, contains('<time>'));
      expect(gpx, contains('<trk>'));
      expect(gpx, contains('<trkseg>'));
      expect(gpx, contains('lat="59.9"'));
      expect(gpx, contains('lon="10.7"'));
      expect(gpx, contains('lat="60.0"'));
      expect(gpx, contains('lon="10.8"'));
      expect(gpx, contains('</gpx>'));
    });

    test('escapes XML special characters in title and description', () {
      final path = _makePath(
        title: 'Trail & Ridge <North>',
        description: 'He said "it\'s great"',
      );
      final gpx = savedPathToGpx(path);

      expect(gpx, contains('Trail &amp; Ridge &lt;North&gt;'));
      expect(gpx, contains('He said &quot;it&apos;s great&quot;'));
      // Must not contain unescaped special characters in name/desc
      expect(
        gpx.indexOf('<name>'),
        lessThan(gpx.indexOf('Trail &amp;')),
      );
    });

    test('omits <desc> when description is null', () {
      final path = _makePath(description: null);
      final gpx = savedPathToGpx(path);

      expect(gpx, isNot(contains('<desc>')));
    });

    test('omits <desc> when description is empty', () {
      final path = _makePath(description: '');
      final gpx = savedPathToGpx(path);

      expect(gpx, isNot(contains('<desc>')));
    });
  });

  group('GeoJSON serializer', () {
    test('produces valid FeatureCollection with LineString', () {
      final path = _makePath(
        title: 'Coastal Walk',
        description: 'Nice walk',
      );
      final json = savedPathToGeoJson(path);
      final parsed = jsonDecode(json) as Map<String, dynamic>;

      expect(parsed['type'], 'FeatureCollection');

      final features = parsed['features'] as List;
      expect(features, hasLength(1));

      final feature = features[0] as Map<String, dynamic>;
      expect(feature['type'], 'Feature');

      final geometry = feature['geometry'] as Map<String, dynamic>;
      expect(geometry['type'], 'LineString');

      final coordinates = geometry['coordinates'] as List;
      expect(coordinates, hasLength(2));
    });

    test('uses [lon, lat] coordinate order per RFC 7946', () {
      final path = _makePath(
        points: [const LatLng(59.9, 10.7)],
      );
      final json = savedPathToGeoJson(path);
      final parsed = jsonDecode(json) as Map<String, dynamic>;
      final feature = (parsed['features'] as List)[0] as Map<String, dynamic>;
      final coords =
          (feature['geometry'] as Map<String, dynamic>)['coordinates'] as List;
      final firstPoint = coords[0] as List;

      // [longitude, latitude]
      expect(firstPoint[0], 10.7); // longitude first
      expect(firstPoint[1], 59.9); // latitude second
    });

    test('includes properties: title, distance, createdAt', () {
      final path = _makePath(
        title: 'My Path',
        distance: 5000.0,
        description: 'A description',
      );
      final json = savedPathToGeoJson(path);
      final parsed = jsonDecode(json) as Map<String, dynamic>;
      final feature = (parsed['features'] as List)[0] as Map<String, dynamic>;
      final props = feature['properties'] as Map<String, dynamic>;

      expect(props['title'], 'My Path');
      expect(props['distance'], 5000.0);
      expect(props['createdAt'], isNotNull);
      expect(props['description'], 'A description');
    });

    test('omits description property when null', () {
      final path = _makePath(description: null);
      final json = savedPathToGeoJson(path);
      final parsed = jsonDecode(json) as Map<String, dynamic>;
      final feature = (parsed['features'] as List)[0] as Map<String, dynamic>;
      final props = feature['properties'] as Map<String, dynamic>;

      expect(props.containsKey('description'), isFalse);
    });
  });

  group('PathExportService.buildFilename', () {
    final service = PathExportService();

    test('produces sanitized filename with date and extension', () {
      final path = _makePath(
        title: 'Besseggen Ridge',
        createdAt: DateTime.utc(2026, 3, 1),
      );

      expect(
        service.buildFilename(path, ExportFormat.gpx),
        'besseggen-ridge-2026-03-01.gpx',
      );
      expect(
        service.buildFilename(path, ExportFormat.geoJson),
        'besseggen-ridge-2026-03-01.geojson',
      );
    });

    test('strips special characters from title', () {
      final path = _makePath(
        title: 'Trail & Ridge (North)',
        createdAt: DateTime.utc(2026, 6, 15),
      );

      expect(
        service.buildFilename(path, ExportFormat.gpx),
        'trail-ridge-north-2026-06-15.gpx',
      );
    });

    test('handles empty title after sanitization', () {
      final path = _makePath(
        title: '!!!',
        createdAt: DateTime.utc(2026, 1, 1),
      );

      expect(
        service.buildFilename(path, ExportFormat.gpx),
        'path-2026-01-01.gpx',
      );
    });

    test('collapses multiple spaces to single hyphen', () {
      final path = _makePath(
        title: 'My    Long   Trail',
        createdAt: DateTime.utc(2026, 12, 25),
      );

      expect(
        service.buildFilename(path, ExportFormat.geoJson),
        'my-long-trail-2026-12-25.geojson',
      );
    });

    test('serialize dispatches to correct serializer', () {
      final path = _makePath(title: 'Test');

      final gpx = service.serialize(path, ExportFormat.gpx);
      expect(gpx, contains('<gpx'));

      final geoJson = service.serialize(path, ExportFormat.geoJson);
      expect(geoJson, contains('FeatureCollection'));
    });
  });
}
