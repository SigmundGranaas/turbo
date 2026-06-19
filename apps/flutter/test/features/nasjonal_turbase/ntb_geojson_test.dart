import 'package:flutter_test/flutter_test.dart';
import 'package:turbo/features/nasjonal_turbase/data/ntb_geojson.dart';

void main() {
  group('NtbGeoJson.point', () {
    test('parses a Point ([lon, lat] order)', () {
      final p = NtbGeoJson.point({
        'type': 'Point',
        'coordinates': [10.5, 59.9],
      });
      expect(p, isNotNull);
      expect(p!.longitude, 10.5);
      expect(p.latitude, 59.9);
    });

    test('unwraps a Feature', () {
      final p = NtbGeoJson.point({
        'type': 'Feature',
        'geometry': {
          'type': 'Point',
          'coordinates': [8.0, 60.0],
        },
      });
      expect(p!.latitude, 60.0);
    });

    test('falls back to the first vertex of a line', () {
      final p = NtbGeoJson.point({
        'type': 'LineString',
        'coordinates': [
          [1.0, 2.0],
          [3.0, 4.0],
        ],
      });
      expect(p!.longitude, 1.0);
      expect(p.latitude, 2.0);
    });

    test('returns null for malformed input', () {
      expect(NtbGeoJson.point(null), isNull);
      expect(NtbGeoJson.point('nope'), isNull);
      expect(NtbGeoJson.point({'type': 'Point', 'coordinates': []}), isNull);
    });
  });

  group('NtbGeoJson.line', () {
    test('parses a LineString in order', () {
      final pts = NtbGeoJson.line({
        'type': 'LineString',
        'coordinates': [
          [10.0, 59.0],
          [10.1, 59.1],
          [10.2, 59.2],
        ],
      });
      expect(pts.length, 3);
      expect(pts.first.latitude, 59.0);
      expect(pts.last.longitude, 10.2);
    });

    test('flattens a MultiLineString', () {
      final pts = NtbGeoJson.line({
        'type': 'MultiLineString',
        'coordinates': [
          [
            [1.0, 1.0],
            [2.0, 2.0],
          ],
          [
            [3.0, 3.0],
          ],
        ],
      });
      expect(pts.length, 3);
    });

    test('empty for a point or garbage', () {
      expect(NtbGeoJson.line({'type': 'Point', 'coordinates': [1, 2]}), isEmpty);
      expect(NtbGeoJson.line(null), isEmpty);
    });
  });
}
