import 'package:flutter_test/flutter_test.dart';
import 'package:latlong2/latlong.dart';
import 'package:turbo/core/sharing/shareable_link_codec.dart';
import 'package:turbo/features/markers/data/marker_export_service.dart';
import 'package:turbo/features/markers/models/marker.dart';

Marker _marker({String title = 'Cabin', String? description = 'In the woods'}) =>
    Marker(
      uuid: 'm1',
      title: title,
      description: description,
      icon: 'home',
      position: const LatLng(60.5, 10.5),
    );

void main() {
  group('MarkerExportService.buildShareLink', () {
    const base = 'https://example.test';
    final service = MarkerExportService();

    test('produces a /share/m URL that round-trips through the codec', () {
      final marker = _marker();
      final url = service.buildShareLink(marker, base);

      expect(url, startsWith('$base/share/m'));
      final decoded = ShareableLinkCodec.decodeShareUrl(Uri.parse(url));
      expect(decoded, isA<SharedMarkerPayload>());
      expect((decoded as SharedMarkerPayload).marker.title, 'Cabin');
    });

    test('preserves description and icon in the encoded payload', () {
      final url = service.buildShareLink(_marker(), base);
      final m = (ShareableLinkCodec.decodeShareUrl(Uri.parse(url))
              as SharedMarkerPayload)
          .marker;
      expect(m.description, 'In the woods');
      expect(m.icon, 'home');
    });
  });
}
