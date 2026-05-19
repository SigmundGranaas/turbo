import 'dart:convert';

import 'package:flutter_test/flutter_test.dart';
import 'package:turbo/core/util/overpass/overpass_to_geojson.dart';

void main() {
  group('OverpassToGeoJson.convert', () {
    test('empty / malformed input returns an empty FeatureCollection', () {
      expect(OverpassToGeoJson.convert('')['features'], isEmpty);
      expect(OverpassToGeoJson.convert('not json')['features'], isEmpty);
      expect(OverpassToGeoJson.convert('[]')['features'], isEmpty);
      expect(OverpassToGeoJson.convert('{"foo": "bar"}')['features'], isEmpty);
    });

    test('way with inline geometry → LineString feature in [lon, lat]', () {
      final body = jsonEncode({
        'version': 0.6,
        'elements': [
          {
            'type': 'way',
            'id': 12345,
            'geometry': [
              {'lat': 60.0, 'lon': 8.0},
              {'lat': 60.1, 'lon': 8.1},
              {'lat': 60.2, 'lon': 8.2},
            ],
            'tags': {
              'highway': 'path',
              'name': 'Besseggen',
              'sac_scale': 'mountain_hiking',
            },
          },
        ],
      });
      final result = OverpassToGeoJson.convert(body);
      final features = result['features'] as List;
      expect(features, hasLength(1));
      final f = features.first as Map;
      expect(f['id'], 'way/12345');
      final geom = f['geometry'] as Map;
      expect(geom['type'], 'LineString');
      expect(geom['coordinates'], [
        [8.0, 60.0],
        [8.1, 60.1],
        [8.2, 60.2],
      ]);
      final props = f['properties'] as Map;
      expect(props['highway'], 'path');
      expect(props['name'], 'Besseggen');
      expect(props['sac_scale'], 'mountain_hiking');
    });

    test('nodes and relations are skipped', () {
      final body = jsonEncode({
        'elements': [
          {'type': 'node', 'id': 1, 'lat': 60.0, 'lon': 8.0, 'tags': {}},
          {'type': 'relation', 'id': 2, 'members': const []},
          {
            'type': 'way',
            'id': 3,
            'geometry': [
              {'lat': 60.0, 'lon': 8.0},
              {'lat': 60.1, 'lon': 8.1},
            ],
            'tags': const {},
          },
        ],
      });
      final features = OverpassToGeoJson.convert(body)['features'] as List;
      expect(features, hasLength(1));
      expect((features.first as Map)['id'], 'way/3');
    });

    test('ways with <2 vertices are dropped', () {
      final body = jsonEncode({
        'elements': [
          {
            'type': 'way',
            'id': 1,
            'geometry': [
              {'lat': 60.0, 'lon': 8.0},
            ],
            'tags': const {},
          },
        ],
      });
      expect(OverpassToGeoJson.convert(body)['features'], isEmpty);
    });

    test('area=yes + closed ring → Polygon', () {
      final body = jsonEncode({
        'elements': [
          {
            'type': 'way',
            'id': 1,
            'geometry': [
              {'lat': 60.0, 'lon': 8.0},
              {'lat': 60.1, 'lon': 8.0},
              {'lat': 60.1, 'lon': 8.1},
              {'lat': 60.0, 'lon': 8.0},
            ],
            'tags': {'leisure': 'park', 'area': 'yes'},
          },
        ],
      });
      final features = OverpassToGeoJson.convert(body)['features'] as List;
      final geom = (features.first as Map)['geometry'] as Map;
      expect(geom['type'], 'Polygon');
    });
  });
}
