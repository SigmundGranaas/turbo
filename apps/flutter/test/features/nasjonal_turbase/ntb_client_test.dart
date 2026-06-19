import 'dart:convert';

import 'package:flutter_test/flutter_test.dart';
import 'package:turbo/features/nasjonal_turbase/data/ntb_client.dart';
import 'package:turbo/features/nasjonal_turbase/models/ntb_poi.dart';

void main() {
  group('buildListUri', () {
    final client = NtbClient(apiKey: 'test-key');

    test('targets the versioned endpoint with the api key and a near query', () {
      final uri = client.buildListUri(
        type: 'steder',
        minLat: 59.0,
        minLon: 10.0,
        maxLat: 60.0,
        maxLon: 11.0,
      );
      expect(uri.host, NtbClient.host);
      expect(uri.path, '/${NtbClient.apiVersion}/steder');
      expect(uri.queryParameters['api_key'], 'test-key');

      final near = jsonDecode(uri.queryParameters['near']!) as Map;
      final coords =
          (near[r'$geometry'] as Map)['coordinates'] as List;
      // GeoJSON order [lon, lat]; centre of the bbox.
      expect(coords[0], closeTo(10.5, 1e-9));
      expect(coords[1], closeTo(59.5, 1e-9));
      expect(near[r'$maxDistance'], isPositive);
    });

    test('passes tag filters when provided', () {
      final uri = client.buildListUri(
        type: 'steder',
        minLat: 59.0,
        minLon: 10.0,
        maxLat: 60.0,
        maxLon: 11.0,
        tags: const ['Hytte'],
      );
      expect(uri.queryParameters['tags'], 'Hytte');
    });
  });

  group('document projections', () {
    test('poiFromSted classifies a Hytte tag as a cabin', () {
      final poi = NtbClient.poiFromSted({
        '_id': 'abc',
        'navn': 'Test cabin',
        'tags': ['Hytte'],
        'geojson': {
          'type': 'Point',
          'coordinates': [10.0, 60.0],
        },
      });
      expect(poi, isNotNull);
      expect(poi!.type, NtbPoiType.cabin);
      expect(poi.title, 'Test cabin');
      expect(poi.position.latitude, 60.0);
      expect(poi.utUrl, 'https://ut.no/hytte/abc');
    });

    test('poiFromSted without Hytte tag is a place', () {
      final poi = NtbClient.poiFromSted({
        '_id': 'p1',
        'navn': 'Viewpoint',
        'tags': ['Utsiktspunkt'],
        'geojson': {
          'type': 'Point',
          'coordinates': [9.0, 61.0],
        },
      });
      expect(poi!.type, NtbPoiType.place);
    });

    test('poiFromSted returns null without geometry', () {
      expect(NtbClient.poiFromSted({'_id': 'x', 'navn': 'No geo'}), isNull);
    });

    test('poiFromTur is a trip and prefers an embedded ut.no link', () {
      final poi = NtbClient.poiFromTur({
        '_id': 't1',
        'navn': 'A hike',
        'lenker': [
          {'url': 'https://ut.no/turforslag/999/a-hike'},
        ],
        'geojson': {
          'type': 'Point',
          'coordinates': [10.0, 60.0],
        },
      });
      expect(poi!.type, NtbPoiType.trip);
      expect(poi.hasRoute, isTrue);
      expect(poi.utUrl, 'https://ut.no/turforslag/999/a-hike');
    });

    test('routeFromTur extracts the polyline and metadata', () {
      final route = NtbClient.routeFromTur({
        '_id': 't2',
        'navn': 'Long hike',
        'distanse': 5400,
        'gradering': 'Middels',
        'geojson': {
          'type': 'LineString',
          'coordinates': [
            [10.0, 60.0],
            [10.1, 60.1],
          ],
        },
      });
      expect(route.points.length, 2);
      expect(route.hasGeometry, isTrue);
      expect(route.distanceMeters, 5400);
      expect(route.grade, 'Middels');
    });
  });

  test('an unconfigured client fetches nothing', () async {
    final client = NtbClient(apiKey: '');
    expect(client.isConfigured, isFalse);
    expect(
      await client.fetchPois(
          minLat: 59, minLon: 10, maxLat: 60, maxLon: 11),
      isEmpty,
    );
    expect(await client.fetchRoute('anything'), isNull);
  });
}
