import 'dart:io';

import 'package:flutter_test/flutter_test.dart';
import 'package:turbo/core/util/gml/gml_to_geojson.dart';

String _fixture(String name) =>
    File('test/core/util/gml/fixtures/$name').readAsStringSync();

void main() {
  group('GmlToGeoJson.convert — synthetic inputs', () {
    test('empty body returns an empty FeatureCollection (no throw)', () {
      expect(GmlToGeoJson.convert(''),
          {'type': 'FeatureCollection', 'features': const []});
    });

    test('malformed XML returns an empty FeatureCollection (no throw)', () {
      const broken = '<wfs:FeatureCollection><not-closed>';
      final result = GmlToGeoJson.convert(broken);
      expect(result['type'], 'FeatureCollection');
      expect(result['features'], isEmpty);
    });

    test('duplicate keys: a real value wins over a placeholder "Ukjent"', () {
      const gml = '''
<wfs:FeatureCollection xmlns:wfs="http://www.opengis.net/wfs/2.0"
                       xmlns:gml="http://www.opengis.net/gml/3.2"
                       xmlns:app="http://example.com/app">
  <wfs:member>
    <app:Trail gml:id="t1">
      <app:fotruteInfo>
        <app:FotruteInfo><app:rutenavn>Ukjent</app:rutenavn></app:FotruteInfo>
        <app:FotruteInfo><app:rutenavn>Ukjent</app:rutenavn></app:FotruteInfo>
        <app:FotruteInfo><app:rutenavn>Besseggen</app:rutenavn></app:FotruteInfo>
      </app:fotruteInfo>
      <app:senterlinje>
        <gml:LineString srsName="urn:ogc:def:crs:EPSG::4326">
          <gml:posList>61.0 8.0 61.1 8.1</gml:posList>
        </gml:LineString>
      </app:senterlinje>
    </app:Trail>
  </wfs:member>
</wfs:FeatureCollection>
''';
      final result = GmlToGeoJson.convert(gml);
      final features = result['features'] as List;
      expect(features, hasLength(1));
      final props = (features.first as Map)['properties'] as Map;
      expect(props['rutenavn'], 'Besseggen');
    });

    test('duplicate keys: first non-placeholder value wins (later '
        'duplicates do not overwrite)', () {
      const gml = '''
<wfs:FeatureCollection xmlns:wfs="http://www.opengis.net/wfs/2.0"
                       xmlns:gml="http://www.opengis.net/gml/3.2"
                       xmlns:app="http://example.com/app">
  <wfs:member>
    <app:Trail gml:id="t2">
      <app:fotruteInfo>
        <app:FotruteInfo><app:rutenavn>Besseggen</app:rutenavn></app:FotruteInfo>
        <app:FotruteInfo><app:rutenavn>Other name</app:rutenavn></app:FotruteInfo>
      </app:fotruteInfo>
      <app:senterlinje>
        <gml:LineString srsName="urn:ogc:def:crs:EPSG::4326">
          <gml:posList>61.0 8.0 61.1 8.1</gml:posList>
        </gml:LineString>
      </app:senterlinje>
    </app:Trail>
  </wfs:member>
</wfs:FeatureCollection>
''';
      final result = GmlToGeoJson.convert(gml);
      final props = ((result['features'] as List).first as Map)['properties']
          as Map;
      expect(props['rutenavn'], 'Besseggen');
    });

    test('LineString with urn-form SRS swaps lat,lon → [lon, lat]', () {
      const gml = '''
<wfs:FeatureCollection
    xmlns:wfs="http://www.opengis.net/wfs/2.0"
    xmlns:gml="http://www.opengis.net/gml/3.2"
    xmlns:app="http://example.test/app">
  <wfs:member>
    <app:Trail gml:id="t.1">
      <app:rutenavn>Test</app:rutenavn>
      <app:senterlinje>
        <gml:LineString srsName="urn:ogc:def:crs:EPSG::4326">
          <gml:posList>61.5 8.1 61.6 8.2 61.7 8.3</gml:posList>
        </gml:LineString>
      </app:senterlinje>
    </app:Trail>
  </wfs:member>
</wfs:FeatureCollection>''';
      final r = GmlToGeoJson.convert(gml);
      final features = r['features'] as List;
      expect(features, hasLength(1));
      final feat = features.first as Map;
      expect(feat['id'], 't.1');
      final geom = feat['geometry'] as Map;
      expect(geom['type'], 'LineString');
      // Input was lat,lon (urn form). Output must be [lon, lat].
      expect(geom['coordinates'], [
        [8.1, 61.5],
        [8.2, 61.6],
        [8.3, 61.7],
      ]);
      expect((feat['properties'] as Map)['rutenavn'], 'Test');
    });

    test('LineString with legacy short-form `EPSG:4326` SRS does NOT swap '
        '(servers interpret short form as lon,lat for backwards compat)',
        () {
      const gml = '''
<wfs:FeatureCollection
    xmlns:wfs="http://www.opengis.net/wfs/2.0"
    xmlns:gml="http://www.opengis.net/gml/3.2"
    xmlns:app="http://example.test/app">
  <wfs:member>
    <app:Trail>
      <app:line>
        <gml:LineString srsName="EPSG:4326">
          <gml:posList>8.1 61.5 8.2 61.6</gml:posList>
        </gml:LineString>
      </app:line>
    </app:Trail>
  </wfs:member>
</wfs:FeatureCollection>''';
      final geom =
          ((GmlToGeoJson.convert(gml)['features'] as List).first as Map)
              ['geometry'] as Map;
      // Input was already lon,lat under the short-form convention —
      // must pass through unchanged.
      expect(geom['coordinates'], [
        [8.1, 61.5],
        [8.2, 61.6],
      ]);
    });

    test('http://www.opengis.net/def/crs/EPSG/0/4326 form also swaps', () {
      const gml = '''
<wfs:FeatureCollection
    xmlns:wfs="http://www.opengis.net/wfs/2.0"
    xmlns:gml="http://www.opengis.net/gml/3.2"
    xmlns:app="http://example.test/app">
  <wfs:member>
    <app:Trail>
      <app:line>
        <gml:LineString srsName="http://www.opengis.net/def/crs/EPSG/0/4326">
          <gml:posList>61.5 8.1 61.6 8.2</gml:posList>
        </gml:LineString>
      </app:line>
    </app:Trail>
  </wfs:member>
</wfs:FeatureCollection>''';
      final geom =
          ((GmlToGeoJson.convert(gml)['features'] as List).first as Map)
              ['geometry'] as Map;
      expect(geom['coordinates'], [
        [8.1, 61.5],
        [8.2, 61.6],
      ]);
    });

    test('UTM zone (EPSG:25833) — coordinates pass through as easting,'
        'northing (no swap)', () {
      const gml = '''
<wfs:FeatureCollection
    xmlns:wfs="http://www.opengis.net/wfs/2.0"
    xmlns:gml="http://www.opengis.net/gml/3.2"
    xmlns:app="http://example.test/app">
  <wfs:member>
    <app:Trail>
      <app:line>
        <gml:LineString srsName="urn:ogc:def:crs:EPSG::25833">
          <gml:posList>500000 6700000 500100 6700100</gml:posList>
        </gml:LineString>
      </app:line>
    </app:Trail>
  </wfs:member>
</wfs:FeatureCollection>''';
      final geom =
          ((GmlToGeoJson.convert(gml)['features'] as List).first as Map)
              ['geometry'] as Map;
      expect(geom['coordinates'], [
        [500000, 6700000],
        [500100, 6700100],
      ]);
    });

    test('Point geometry from gml:pos', () {
      const gml = '''
<wfs:FeatureCollection
    xmlns:wfs="http://www.opengis.net/wfs/2.0"
    xmlns:gml="http://www.opengis.net/gml/3.2"
    xmlns:app="http://example.test/app">
  <wfs:member>
    <app:Marker>
      <app:loc>
        <gml:Point srsName="urn:ogc:def:crs:EPSG::4326">
          <gml:pos>61.5 8.1</gml:pos>
        </gml:Point>
      </app:loc>
    </app:Marker>
  </wfs:member>
</wfs:FeatureCollection>''';
      final geom =
          ((GmlToGeoJson.convert(gml)['features'] as List).first as Map)
              ['geometry'] as Map;
      expect(geom['type'], 'Point');
      expect(geom['coordinates'], [8.1, 61.5]);
    });

    test('MultiCurve produces MultiLineString with one ring per LineString',
        () {
      const gml = '''
<wfs:FeatureCollection
    xmlns:wfs="http://www.opengis.net/wfs/2.0"
    xmlns:gml="http://www.opengis.net/gml/3.2"
    xmlns:app="http://example.test/app">
  <wfs:member>
    <app:Trail>
      <app:line>
        <gml:MultiCurve srsName="urn:ogc:def:crs:EPSG::4326">
          <gml:curveMember>
            <gml:LineString>
              <gml:posList>61.5 8.1 61.6 8.2</gml:posList>
            </gml:LineString>
          </gml:curveMember>
          <gml:curveMember>
            <gml:LineString>
              <gml:posList>62.0 9.0 62.1 9.1 62.2 9.2</gml:posList>
            </gml:LineString>
          </gml:curveMember>
        </gml:MultiCurve>
      </app:line>
    </app:Trail>
  </wfs:member>
</wfs:FeatureCollection>''';
      final geom =
          ((GmlToGeoJson.convert(gml)['features'] as List).first as Map)
              ['geometry'] as Map;
      expect(geom['type'], 'MultiLineString');
      expect(geom['coordinates'], [
        [
          [8.1, 61.5],
          [8.2, 61.6],
        ],
        [
          [9.0, 62.0],
          [9.1, 62.1],
          [9.2, 62.2],
        ],
      ]);
    });

    test('Polygon with exterior + interior rings', () {
      const gml = '''
<wfs:FeatureCollection
    xmlns:wfs="http://www.opengis.net/wfs/2.0"
    xmlns:gml="http://www.opengis.net/gml/3.2"
    xmlns:app="http://example.test/app">
  <wfs:member>
    <app:Area>
      <app:area>
        <gml:Polygon srsName="urn:ogc:def:crs:EPSG::4326">
          <gml:exterior>
            <gml:LinearRing>
              <gml:posList>60 5 60 6 61 6 61 5 60 5</gml:posList>
            </gml:LinearRing>
          </gml:exterior>
          <gml:interior>
            <gml:LinearRing>
              <gml:posList>60.2 5.2 60.2 5.4 60.4 5.4 60.4 5.2 60.2 5.2</gml:posList>
            </gml:LinearRing>
          </gml:interior>
        </gml:Polygon>
      </app:area>
    </app:Area>
  </wfs:member>
</wfs:FeatureCollection>''';
      final geom =
          ((GmlToGeoJson.convert(gml)['features'] as List).first as Map)
              ['geometry'] as Map;
      expect(geom['type'], 'Polygon');
      final rings = geom['coordinates'] as List;
      expect(rings, hasLength(2));
      expect(rings[0].first, [5.0, 60.0]);
      expect(rings[1].first, [5.2, 60.2]);
    });

    test('property keys are stripped of namespace prefixes', () {
      const gml = '''
<wfs:FeatureCollection
    xmlns:wfs="http://www.opengis.net/wfs/2.0"
    xmlns:gml="http://www.opengis.net/gml/3.2"
    xmlns:app="http://example.test/app">
  <wfs:member>
    <app:Trail>
      <app:rutenavn>Besseggen</app:rutenavn>
      <app:line>
        <gml:LineString srsName="urn:ogc:def:crs:EPSG::4326">
          <gml:posList>61.5 8.1 61.6 8.2</gml:posList>
        </gml:LineString>
      </app:line>
    </app:Trail>
  </wfs:member>
</wfs:FeatureCollection>''';
      final props = ((GmlToGeoJson.convert(gml)['features'] as List).first
          as Map)['properties'] as Map;
      expect(props.containsKey('rutenavn'), isTrue);
      expect(props.keys.any((k) => '$k'.contains(':')), isFalse,
          reason: 'no app:/gml: prefixes should leak into property keys');
    });

    test('feature with no parseable geometry is skipped; valid siblings '
        'still emit', () {
      const gml = '''
<wfs:FeatureCollection
    xmlns:wfs="http://www.opengis.net/wfs/2.0"
    xmlns:gml="http://www.opengis.net/gml/3.2"
    xmlns:app="http://example.test/app">
  <wfs:member>
    <app:Trail gml:id="empty"><app:rutenavn>NoGeom</app:rutenavn></app:Trail>
  </wfs:member>
  <wfs:member>
    <app:Trail gml:id="ok">
      <app:rutenavn>HasGeom</app:rutenavn>
      <app:line>
        <gml:LineString srsName="urn:ogc:def:crs:EPSG::4326">
          <gml:posList>61.5 8.1 61.6 8.2</gml:posList>
        </gml:LineString>
      </app:line>
    </app:Trail>
  </wfs:member>
</wfs:FeatureCollection>''';
      final features = GmlToGeoJson.convert(gml)['features'] as List;
      expect(features, hasLength(1));
      expect((features.first as Map)['id'], 'ok');
    });

    test('gml:id becomes feature.id; missing id auto-numbers', () {
      const gml = '''
<wfs:FeatureCollection
    xmlns:wfs="http://www.opengis.net/wfs/2.0"
    xmlns:gml="http://www.opengis.net/gml/3.2"
    xmlns:app="http://example.test/app">
  <wfs:member>
    <app:Trail>
      <app:line>
        <gml:LineString srsName="urn:ogc:def:crs:EPSG::4326">
          <gml:posList>61.5 8.1 61.6 8.2</gml:posList>
        </gml:LineString>
      </app:line>
    </app:Trail>
  </wfs:member>
</wfs:FeatureCollection>''';
      final id = ((GmlToGeoJson.convert(gml)['features'] as List).first
          as Map)['id'];
      expect(id, startsWith('feat-'));
    });

    test('returns the GeoJSON envelope even when no features are present',
        () {
      const gml = '''
<wfs:FeatureCollection
    xmlns:wfs="http://www.opengis.net/wfs/2.0"
    xmlns:gml="http://www.opengis.net/gml/3.2"
    numberReturned="0" numberMatched="unknown" />''';
      expect(GmlToGeoJson.convert(gml),
          {'type': 'FeatureCollection', 'features': const []});
    });
  });

  group('GmlToGeoJson.convert — captured live WFS payloads', () {
    test('single fotrute (real Geonorge payload, urn EPSG::4326)', () {
      final result = GmlToGeoJson.convert(_fixture('fotrute_one.gml'));
      final features = result['features'] as List;
      expect(features, hasLength(1));

      final f = features.first as Map;
      expect(f['id'], 'fotrute.100128');
      final geom = f['geometry'] as Map;
      expect(geom['type'], 'LineString');
      final coords = geom['coordinates'] as List;
      expect(coords.length, greaterThanOrEqualTo(2));

      // First pair: original posList was "61.673901 8.375147 …" (lat,lon
      // under urn form). After axis normalisation we expect [lon, lat].
      expect(coords.first, [closeTo(8.375147, 1e-6), closeTo(61.673901, 1e-6)]);

      // All coordinates should be in Norway's bbox.
      for (final p in coords) {
        final lon = (p as List)[0] as double;
        final lat = p[1] as double;
        expect(lat, inInclusiveRange(57.0, 72.0),
            reason: 'lat out of Norway range — axis swap broken?');
        expect(lon, inInclusiveRange(4.0, 32.0),
            reason: 'lon out of Norway range — axis swap broken?');
      }

      final props = f['properties'] as Map;
      expect(props['rutenummer'], isNotNull);
      // SOSI uses "rutenavn"; the (imaginary) GeoJSON shape the PR
      // assumed used "navn". Document the real field name.
      expect(props['rutenavn'], isNotNull);

      // Regression: the fixture has three <app:FotruteInfo> blocks with
      // rutenavn "Ukjent", "Ukjent", "Bøverdalen" respectively. Before
      // the duplicate-key fix, the first "Ukjent" shadowed everything;
      // now the real name wins.
      expect(props['rutenavn'], 'Bøverdalen',
          reason: 'duplicate-key resolution must let a real name beat '
              'an earlier placeholder');
    });

    test('three fotrute features each have a usable geometry', () {
      final result = GmlToGeoJson.convert(_fixture('fotrute_three.gml'));
      final features = result['features'] as List;
      expect(features, hasLength(3));
      final ids = features.map((f) => (f as Map)['id']).toList();
      expect(ids.toSet(), hasLength(3),
          reason: 'each feature must have a distinct id');
      for (final f in features) {
        final geom = (f as Map)['geometry'] as Map;
        expect(geom['type'], anyOf('LineString', 'MultiLineString'));
        final coords = geom['coordinates'] as List;
        expect(coords, isNotEmpty);
      }
    });

    test('empty result envelope produces an empty FeatureCollection', () {
      final result = GmlToGeoJson.convert(_fixture('empty.gml'));
      expect(result, {
        'type': 'FeatureCollection',
        'features': const [],
      });
    });

    test('skiløype feature uses the same parser shape', () {
      final result = GmlToGeoJson.convert(_fixture('skiloype.gml'));
      final features = result['features'] as List;
      expect(features, isNotEmpty);
      final geom = (features.first as Map)['geometry'] as Map;
      expect(geom['type'], 'LineString');
    });
  });
}
