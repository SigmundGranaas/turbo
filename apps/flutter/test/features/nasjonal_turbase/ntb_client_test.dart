import 'package:flutter_test/flutter_test.dart';
import 'package:turbo/features/nasjonal_turbase/data/ntb_client.dart';
import 'package:turbo/features/nasjonal_turbase/models/ntb_poi.dart';

void main() {
  group('poisUri', () {
    final client = NtbClient(baseUrl: 'https://api.example.com');

    test('targets the proxy with bbox query params', () {
      final uri = client.poisUri(minLat: 59.0, minLon: 10.0, maxLat: 60.0, maxLon: 11.0);
      expect(uri.path, '/api/places/ntb/pois');
      expect(uri.queryParameters['minLat'], '59.0');
      expect(uri.queryParameters['maxLon'], '11.0');
    });
  });

  group('poiFromJson', () {
    test('maps a cabin DTO', () {
      final poi = NtbClient.poiFromJson({
        'id': 'abc',
        'type': 'cabin',
        'lat': 60.0,
        'lng': 10.0,
        'title': 'Test cabin',
        'utUrl': 'https://ut.no/hytte/abc',
      });
      expect(poi, isNotNull);
      expect(poi!.type, NtbPoiType.cabin);
      expect(poi.title, 'Test cabin');
      expect(poi.position.latitude, 60.0);
      expect(poi.utUrl, 'https://ut.no/hytte/abc');
    });

    test('maps a trip DTO and reports hasRoute', () {
      final poi = NtbClient.poiFromJson({
        'id': 't1',
        'type': 'trip',
        'lat': 60.0,
        'lng': 10.0,
        'title': 'A hike',
      });
      expect(poi!.type, NtbPoiType.trip);
      expect(poi.hasRoute, isTrue);
    });

    test('unknown type falls back to place', () {
      final poi = NtbClient.poiFromJson(
          {'id': 'x', 'type': 'whatever', 'lat': 1.0, 'lng': 2.0, 'title': 'X'});
      expect(poi!.type, NtbPoiType.place);
    });

    test('returns null without coordinates', () {
      expect(NtbClient.poiFromJson({'id': 'x', 'title': 'No geo'}), isNull);
    });
  });

  group('routeFromJson', () {
    test('parses [lng,lat] points into LatLng and metadata', () {
      final route = NtbClient.routeFromJson({
        'id': 't2',
        'title': 'Long hike',
        'distanceMeters': 5400,
        'grade': 'Middels',
        'points': [
          [10.0, 60.0],
          [10.1, 60.1],
        ],
      });
      expect(route.points.length, 2);
      expect(route.hasGeometry, isTrue);
      expect(route.points.first.latitude, 60.0);
      expect(route.points.first.longitude, 10.0);
      expect(route.distanceMeters, 5400);
      expect(route.grade, 'Middels');
    });

    test('tolerates missing/garbage points', () {
      final route = NtbClient.routeFromJson({'id': 'x', 'title': 'X'});
      expect(route.points, isEmpty);
      expect(route.hasGeometry, isFalse);
    });
  });
}
