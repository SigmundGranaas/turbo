import 'package:flutter_test/flutter_test.dart';
import 'package:latlong2/latlong.dart';
import 'package:turbo/core/sharing/shareable_link_codec.dart';
import 'package:turbo/features/markers/api.dart';
import 'package:turbo/features/saved_paths/api.dart';

const _base = 'https://example.test';

Marker _marker({
  String title = 'Trolltunga',
  String? description = 'A cliff in Norway',
  String? icon = 'mountain',
  LatLng? position,
}) {
  return Marker(
    uuid: 'm-uuid',
    title: title,
    description: description,
    icon: icon,
    position: position ?? const LatLng(60.124123, 6.740456),
  );
}

SavedPath _path({
  int points = 3,
  String title = 'Hike',
  String? description = 'A long walk',
  String? colorHex = '#ff5500',
  String? iconKey = 'flag',
  bool smoothing = true,
  String? lineStyleKey = 'dashed',
}) {
  final pts = <LatLng>[
    for (var i = 0; i < points; i++)
      LatLng(60.0 + i * 0.0001, 10.0 + i * 0.0001),
  ];
  return SavedPath(
    uuid: 'p-uuid',
    title: title,
    description: description,
    points: pts,
    distance: 1234.5,
    createdAt: DateTime.utc(2025, 5, 1, 12),
    colorHex: colorHex,
    iconKey: iconKey,
    smoothing: smoothing,
    lineStyleKey: lineStyleKey,
  );
}

void main() {
  group('encodeMarker / decodeShareUrl', () {
    test('round-trips a marker with all fields', () {
      final marker = _marker();
      final url = ShareableLinkCodec.encodeMarker(marker, _base);
      final decoded = ShareableLinkCodec.decodeShareUrl(Uri.parse(url));

      expect(decoded, isA<SharedMarkerPayload>());
      final m = (decoded as SharedMarkerPayload).marker;
      expect(m.title, marker.title);
      expect(m.description, marker.description);
      expect(m.icon, marker.icon);
      expect(m.position.latitude, closeTo(marker.position.latitude, 1e-5));
      expect(m.position.longitude, closeTo(marker.position.longitude, 1e-5));
    });

    test('round-trips a marker with only required fields', () {
      final marker = _marker(description: null, icon: null);
      final url = ShareableLinkCodec.encodeMarker(marker, _base);
      final decoded = ShareableLinkCodec.decodeShareUrl(Uri.parse(url));
      final m = (decoded as SharedMarkerPayload).marker;

      expect(m.title, marker.title);
      expect(m.description, isNull);
      expect(m.icon, isNull);
    });

    test('produces a URL pointing at the configured base /share/m', () {
      final url = ShareableLinkCodec.encodeMarker(_marker(), _base);
      final uri = Uri.parse(url);

      expect(uri.scheme, 'https');
      expect(uri.host, 'example.test');
      expect(uri.path, '/share/m');
      expect(uri.queryParameters['d'], isNotNull);
      expect(uri.queryParameters['d'], isNotEmpty);
    });

    test('uses URL-safe base64 (no +, /, or = in payload)', () {
      // Force a payload that's likely to need padding by varying lengths.
      for (var i = 1; i < 30; i++) {
        final url = ShareableLinkCodec.encodeMarker(
          _marker(title: 'x' * i),
          _base,
        );
        final d = Uri.parse(url).queryParameters['d']!;
        expect(d.contains('+'), isFalse, reason: 'contains + in $d');
        expect(d.contains('/'), isFalse, reason: 'contains / in $d');
        expect(d.contains('='), isFalse, reason: 'contains = in $d');
      }
    });
  });

  group('encodePath / decodeShareUrl', () {
    test('round-trips a path with all fields', () {
      final path = _path(points: 5);
      final url = ShareableLinkCodec.encodePath(path, _base);
      final decoded = ShareableLinkCodec.decodeShareUrl(Uri.parse(url));

      expect(decoded, isA<SharedPathPayload>());
      final p = (decoded as SharedPathPayload).path;
      expect(p.title, path.title);
      expect(p.description, path.description);
      expect(p.colorHex, path.colorHex);
      expect(p.iconKey, path.iconKey);
      expect(p.smoothing, path.smoothing);
      expect(p.lineStyleKey, path.lineStyleKey);
      expect(p.points.length, path.points.length);
      for (var i = 0; i < path.points.length; i++) {
        expect(p.points[i].latitude,
            closeTo(path.points[i].latitude, 1e-5));
        expect(p.points[i].longitude,
            closeTo(path.points[i].longitude, 1e-5));
      }
    });

    test('round-trips a path with only required fields', () {
      final path = _path(
        description: null,
        colorHex: null,
        iconKey: null,
        smoothing: false,
        lineStyleKey: null,
      );
      final url = ShareableLinkCodec.encodePath(path, _base);
      final decoded = ShareableLinkCodec.decodeShareUrl(Uri.parse(url));
      final p = (decoded as SharedPathPayload).path;

      expect(p.description, isNull);
      expect(p.colorHex, isNull);
      expect(p.iconKey, isNull);
      expect(p.smoothing, isFalse);
      expect(p.lineStyleKey, isNull);
    });

    test('produces a URL pointing at the configured base /share/p', () {
      final url = ShareableLinkCodec.encodePath(_path(), _base);
      final uri = Uri.parse(url);

      expect(uri.path, '/share/p');
      expect(uri.queryParameters['d'], isNotEmpty);
    });

    test('throws LinkTooLargeException for very large paths', () {
      final huge = _path(points: 100000);
      expect(
        () => ShareableLinkCodec.encodePath(huge, _base),
        throwsA(isA<LinkTooLargeException>()),
      );
    });

    test('handles a base URL with a trailing slash', () {
      final url = ShareableLinkCodec.encodeMarker(
        _marker(),
        'https://example.test/',
      );
      expect(Uri.parse(url).path, '/share/m');
    });

    test('handles a base URL with a path prefix', () {
      final url = ShareableLinkCodec.encodeMarker(
        _marker(),
        'https://example.test/app',
      );
      expect(Uri.parse(url).path, '/app/share/m');
    });
  });

  group('decodeShareUrl error cases', () {
    test('returns null for an unrelated path', () {
      final result = ShareableLinkCodec.decodeShareUrl(
        Uri.parse('https://example.test/login'),
      );
      expect(result, isNull);
    });

    test('throws for /share/m with a missing data parameter', () {
      expect(
        () => ShareableLinkCodec.decodeShareUrl(
          Uri.parse('https://example.test/share/m'),
        ),
        throwsA(isA<InvalidShareLinkException>()),
      );
    });

    test('throws for /share/p with corrupt base64 data', () {
      expect(
        () => ShareableLinkCodec.decodeShareUrl(
          Uri.parse('https://example.test/share/p?d=not-base64!!!'),
        ),
        throwsA(isA<InvalidShareLinkException>()),
      );
    });

    test('throws for an unknown payload version', () {
      // Hand-craft a payload with v: 99 to confirm forward-compat handling.
      final url = ShareableLinkCodec.encodeMarker(_marker(), _base);
      final goodData = Uri.parse(url).queryParameters['d']!;
      // Build a fake URL with the wrong path style won't trigger this; we
      // instead exercise the version check via the test-only helper.
      expect(
        () => ShareableLinkCodec.decodeRawPayload(goodData, kind: 'm',
            overrideVersionForTest: 99),
        throwsA(isA<UnsupportedShareVersionException>()),
      );
    });
  });
}
