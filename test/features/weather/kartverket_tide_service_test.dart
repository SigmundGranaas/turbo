import 'dart:io';

import 'package:flutter_test/flutter_test.dart';
import 'package:http/http.dart' as http;
import 'package:http/testing.dart';
import 'package:latlong2/latlong.dart';
import 'package:turbo/core/util/user_agent.dart';
import 'package:turbo/features/weather/api.dart';

void main() {
  group('KartverketTideService.fetch', () {
    test('parses the sample XML into a sorted list of extrema', () async {
      final xml =
          await File('test/features/weather/fixtures/tide_sample.xml')
              .readAsString();
      http.Request? captured;
      final client = MockClient((req) async {
        captured = req;
        return http.Response(xml, 200, headers: const {
          'content-type': 'application/xml; charset=utf-8',
        });
      });
      final service = KartverketTideService(client: client);

      final result =
          await service.fetch(const LatLng(60.3979, 5.3221));

      expect(result, isNotNull);
      expect(captured!.url.host, 'vannstand.kartverket.no');
      expect(captured!.headers['User-Agent'], kTurboUserAgent);
      expect(result!.stationName, 'Bergen');
      expect(result.extrema, hasLength(4));
      // First entry is a "high".
      expect(result.extrema.first.kind, TideKind.high);
      // List is sorted ascending by time.
      for (var i = 1; i < result.extrema.length; i++) {
        expect(
          result.extrema[i].timeUtc
              .isAfter(result.extrema[i - 1].timeUtc),
          isTrue,
        );
      }
    });

    test('returns null when out of coverage (HTTP error)', () async {
      final client =
          MockClient((_) async => http.Response('boom', 500));
      final service = KartverketTideService(client: client);
      final result =
          await service.fetch(const LatLng(0, 0));
      expect(result, isNull);
    });

    test('returns null when XML has no waterlevel rows', () async {
      const empty = '<?xml version="1.0"?><tide><locationdata/></tide>';
      final client = MockClient((_) async =>
          http.Response(empty, 200, headers: const {
            'content-type': 'application/xml',
          }));
      final service = KartverketTideService(client: client);
      final result =
          await service.fetch(const LatLng(0, 0));
      expect(result, isNull);
    });

    test('returns null on malformed XML', () async {
      final client = MockClient(
          (_) async => http.Response('not xml at all', 200));
      final service = KartverketTideService(client: client);
      final result = await service.fetch(const LatLng(0, 0));
      expect(result, isNull);
    });

    test('fromtime/totime are formatted in Europe/Oslo, not the device tz',
        () async {
      // The sehavniva endpoint interprets unzoned timestamps as Norwegian
      // local time. A device in a non-Norway timezone must still produce a
      // Norway-time window. We assert the URL's fromtime matches what
      // Europe/Oslo should be for `now - 6h`, within a generous tolerance to
      // account for time elapsed during the call.
      Uri? captured;
      final client = MockClient((req) async {
        captured = req.url;
        return http.Response(
          '<?xml version="1.0"?><tide><locationdata/></tide>',
          200,
        );
      });
      await KartverketTideService(client: client)
          .fetch(const LatLng(60.4, 5.32));

      final fromtime = captured!.queryParameters['fromtime']!;
      final sent = DateTime.parse(fromtime); // unzoned → local DateTime
      // What Oslo wall-clock time _should_ be for (UTC now − 6h):
      final utc = DateTime.now().toUtc().subtract(const Duration(hours: 6));
      // Manual Oslo offset (DST: last Sunday of March 01:00 UTC ↔ last
      // Sunday of October 01:00 UTC). Matches the service implementation.
      DateTime lastSun(int y, int m) {
        final lastDay = DateTime.utc(y, m + 1, 0);
        return lastDay.subtract(Duration(days: lastDay.weekday % 7));
      }
      final dstStart = lastSun(utc.year, 3).add(const Duration(hours: 1));
      final dstEnd = lastSun(utc.year, 10).add(const Duration(hours: 1));
      final isDst = utc.isAfter(dstStart) && utc.isBefore(dstEnd);
      final expectedOslo = utc.add(Duration(hours: isDst ? 2 : 1));

      final expectedNaive = DateTime(
        expectedOslo.year,
        expectedOslo.month,
        expectedOslo.day,
        expectedOslo.hour,
        expectedOslo.minute,
      );
      // ±2 minutes catches elapsed wall-time in the test without permitting
      // a full hour of timezone drift.
      expect(sent.difference(expectedNaive).inMinutes.abs(), lessThan(2));
    });
  });
}
