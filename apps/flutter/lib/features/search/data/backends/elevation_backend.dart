import 'package:latlong2/latlong.dart';

import 'package:turbo/core/api/kartverket_hoydedata_client.dart';

/// Thin facade over [KartverketHoydedataClient] for the reverse-geocode
/// orchestrator's enrichment side-call ("Galdhøpiggen, 2469 m"). The
/// orchestrator only needs the single-point form, so we expose just that
/// — the shared client handles the HTTP, parsing, and failure-to-null
/// contract.
class ElevationBackend {
  final KartverketHoydedataClient _client;

  ElevationBackend({KartverketHoydedataClient? client})
      : _client = client ?? KartverketHoydedataClient();

  Future<double?> elevationAt(LatLng coord) => _client.elevationAt(coord);
}
