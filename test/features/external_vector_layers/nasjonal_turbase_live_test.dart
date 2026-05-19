@Tags(['live'])
library;

import 'package:flutter_test/flutter_test.dart';
import 'package:http/http.dart' as http;
import 'package:turbo/features/external_vector_layers/api.dart';

/// Live network test for Nasjonal turbase. Skipped in normal runs.
/// Run with: `flutter test --run-skipped --tags=live test/features/external_vector_layers/nasjonal_turbase_live_test.dart`
///
/// The vector source is currently [VectorLayerSource.disabled] = true,
/// because Geonorge's Turrutebasen WFS only emits GML 3.2.1 (no GeoJSON)
/// and [VectorLayerFetcher] only parses GeoJSON. This test pins two
/// facts that future work will need to honour:
///   1. The fetcher honours `disabled` and short-circuits before any HTTP
///      call (so a network outage never surfaces as a layer error).
///   2. The URL we'd hit if/when re-enabled resolves on the real server
///      (HTTP 200, returns GML — not the "UKJENT APPLIKASJON" the
///      original `wfs.friluftsruter2` URL produced).
void main() {
  const minLat = 61.5;
  const minLon = 8.1;
  const maxLat = 61.8;
  const maxLon = 8.6;

  test('disabled fetcher makes zero requests', () async {
    var calls = 0;
    final client = http.Client();
    addTearDown(client.close);
    // Wrap to count calls.
    final tracking = _CountingClient(client, (_) => calls++);
    final fetcher = VectorLayerFetcher(client: tracking);
    final features = await fetcher.fetchBounds(
      trailVectorSource(TrailSubtype.foot),
      minLat: minLat,
      minLon: minLon,
      maxLat: maxLat,
      maxLon: maxLon,
    );
    expect(features, isEmpty);
    expect(calls, 0);
  });

  test('canonical WFS URL resolves on the live server (HTTP 200, GML)',
      () async {
    final source = trailVectorSource(TrailSubtype.foot);
    final uri = source.buildUri(
      minLat: minLat,
      minLon: minLon,
      maxLat: maxLat,
      maxLon: maxLon,
      maxFeatures: 1,
    );
    final response = await http.get(uri);
    expect(response.statusCode, 200,
        reason: 'Canonical /skwms1/wfs.turogfriluftsruter must resolve. '
            'The PR\'s original /skwms1/wfs.friluftsruter2 returned '
            '"UKJENT APPLIKASJON".');
    // Decode from bodyBytes — the server's content-type
    // (`text/xml; subtype=gml/3.2.1;charset=UTF-8`) trips MediaType.parse
    // in the http package's `.body` getter.
    final body = String.fromCharCodes(response.bodyBytes);
    expect(body, contains('FeatureCollection'),
        reason: 'Body must be a WFS GML FeatureCollection.');
  });
}

class _CountingClient extends http.BaseClient {
  final http.Client _inner;
  final void Function(Uri url) _onSend;
  _CountingClient(this._inner, this._onSend);
  @override
  Future<http.StreamedResponse> send(http.BaseRequest request) {
    _onSend(request.url);
    return _inner.send(request);
  }
}
