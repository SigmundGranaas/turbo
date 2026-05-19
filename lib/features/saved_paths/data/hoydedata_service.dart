import 'package:latlong2/latlong.dart';

import 'package:turbo/core/api/kartverket_hoydedata_client.dart';

/// Re-exported for callers that catch this specifically (the elevation
/// backfill pipeline distinguishes "failed" from "skipped").
typedef HoydedataServiceException = KartverketHoydedataException;

/// Thin facade over [KartverketHoydedataClient] for the import-time
/// elevation backfill. Wraps the shared batch endpoint so saved-paths
/// has a stable, narrow surface; the underlying HTTP, request shape,
/// and rate-limit pacing live in `core/api/` and are shared with the
/// reverse-geocode single-point use case.
class HoydedataService {
  final KartverketHoydedataClient _client;

  HoydedataService({KartverketHoydedataClient? client})
      : _client = client ?? KartverketHoydedataClient();

  Future<List<double?>> elevationsFor(List<LatLng> points) =>
      _client.elevationsFor(points);
}
