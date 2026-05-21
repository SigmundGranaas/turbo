import 'package:flutter_test/flutter_test.dart';
import 'package:turbo/features/saved_paths/api.dart';

const _validGpx = '''
<?xml version="1.0" encoding="UTF-8"?>
<gpx version="1.1" creator="test" xmlns="http://www.topografix.com/GPX/1/1">
  <metadata>
    <name>Sample Hike</name>
    <desc>Test description</desc>
  </metadata>
  <trk>
    <name>First Track</name>
    <trkseg>
      <trkpt lat="61.50" lon="8.75"><ele>1000</ele></trkpt>
      <trkpt lat="61.51" lon="8.77"><ele>1050</ele></trkpt>
      <trkpt lat="61.52" lon="8.80"><ele>1100</ele></trkpt>
    </trkseg>
  </trk>
</gpx>
''';

const _multiTrackGpx = '''
<?xml version="1.0"?>
<gpx version="1.1" creator="test" xmlns="http://www.topografix.com/GPX/1/1">
  <trk><name>A</name><trkseg>
    <trkpt lat="60" lon="10"/><trkpt lat="60.1" lon="10.1"/>
  </trkseg></trk>
  <trk><name>B</name><trkseg>
    <trkpt lat="61" lon="11"/><trkpt lat="61.1" lon="11.1"/>
  </trkseg></trk>
</gpx>
''';

const _gpxNoElevation = '''
<?xml version="1.0"?>
<gpx version="1.1" xmlns="http://www.topografix.com/GPX/1/1"><trk><trkseg>
  <trkpt lat="59.9" lon="10.7"/><trkpt lat="60.0" lon="10.8"/>
</trkseg></trk></gpx>
''';

const _geoJsonLineString = '''
{
  "type": "FeatureCollection",
  "features": [{
    "type": "Feature",
    "properties": {"title": "Imported", "description": "GeoJSON test"},
    "geometry": {
      "type": "LineString",
      "coordinates": [
        [10.7, 59.9, 100],
        [10.8, 60.0, 120],
        [10.9, 60.1, 110]
      ]
    }
  }]
}
''';

const _geoJsonMultiLineString = '''
{
  "type": "Feature",
  "properties": {"name": "Branched"},
  "geometry": {
    "type": "MultiLineString",
    "coordinates": [
      [[10.0, 60.0], [10.1, 60.1]],
      [[11.0, 61.0], [11.1, 61.1], [11.2, 61.2]]
    ]
  }
}
''';

const _kml = '''
<?xml version="1.0" encoding="UTF-8"?>
<kml xmlns="http://www.opengis.net/kml/2.2"><Document>
  <Placemark><name>KML Track</name><description>desc</description>
    <LineString>
      <coordinates>
        10.7,59.9,100 10.8,60.0,120 10.9,60.1,110
      </coordinates>
    </LineString>
  </Placemark>
</Document></kml>
''';

void main() {
  group('parseGpx', () {
    test('extracts title, description, points, and elevation', () {
      final paths = parseGpx(_validGpx);
      expect(paths.length, 1);
      final p = paths.first;
      expect(p.title, 'First Track');
      expect(p.points.length, 3);
      expect(p.elevations, [1000.0, 1050.0, 1100.0]);
      expect(p.ascent, greaterThan(0));
      expect(p.distance, greaterThan(0));
    });

    test('multi-track GPX produces one path per track', () {
      final paths = parseGpx(_multiTrackGpx);
      expect(paths.length, 2);
      expect(paths[0].title, 'A');
      expect(paths[1].title, 'B');
    });

    test('GPX without <ele> tags has null elevations', () {
      final paths = parseGpx(_gpxNoElevation);
      expect(paths.length, 1);
      expect(paths.first.elevations, isNull);
      expect(paths.first.ascent, isNull);
    });

    test('malformed XML throws FormatException', () {
      expect(() => parseGpx('not xml at all'),
          throwsA(isA<FormatException>()));
    });
  });

  group('parseGeoJson', () {
    test('FeatureCollection with LineString round-trips title and elevations',
        () {
      final paths = parseGeoJson(_geoJsonLineString);
      expect(paths.length, 1);
      final p = paths.first;
      expect(p.title, 'Imported');
      expect(p.description, 'GeoJSON test');
      expect(p.points.length, 3);
      expect(p.elevations, [100.0, 120.0, 110.0]);
    });

    test('MultiLineString becomes one path per line', () {
      final paths = parseGeoJson(_geoJsonMultiLineString);
      expect(paths.length, 2);
      expect(paths[0].title, 'Branched');
      expect(paths[1].title, 'Branched');
      expect(paths[0].points.length, 2);
      expect(paths[1].points.length, 3);
    });

    test('malformed JSON throws FormatException', () {
      expect(() => parseGeoJson('{not json'),
          throwsA(isA<FormatException>()));
    });
  });

  group('parseKml', () {
    test('extracts LineString with elevation', () {
      final paths = parseKml(_kml);
      expect(paths.length, 1);
      expect(paths.first.title, 'KML Track');
      expect(paths.first.points.length, 3);
      expect(paths.first.elevations, [100.0, 120.0, 110.0]);
    });
  });

  group('importPathContent', () {
    test('dispatches by file extension', () {
      expect(importPathContent(_validGpx, filename: 'a.gpx').length, 1);
      expect(importPathContent(_geoJsonLineString, filename: 'b.geojson').length,
          1);
      expect(importPathContent(_kml, filename: 'c.kml').length, 1);
    });

    test('sniffs format when extension is unknown', () {
      expect(importPathContent(_validGpx).length, 1);
      expect(importPathContent(_geoJsonLineString).length, 1);
    });

    test('throws PathImportException for unrecognised content', () {
      expect(() => importPathContent('plain text body'),
          throwsA(isA<PathImportException>()));
    });
  });
}
