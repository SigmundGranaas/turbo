import 'package:latlong2/latlong.dart';

import 'backends/kommune_backend.dart';
import 'backends/protected_area_backend.dart';
import 'backends/stedsnavn_backend.dart';
import 'location_service.dart';
import 'reverse_geocoder.dart';

/// Default Norway-first [ReverseGeocoder]. Composes three independent
/// backends:
///
///   1. **Stedsnavn** (`/stedsnavn/v1/punkt`) — toponyms: peaks,
///      settlements, water, farms. Scored by tier (`exactContact` /
///      `inSettlement` / `closeToPeak` / `periphery`).
///   2. **Protected area** (Miljødirektoratet Vern Identify) — national
///      parks, nature reserves, landscape-protected areas. Required for
///      "In Saltfjellet–Svartisen nasjonalpark" to ever resolve, since
///      Stedsnavn doesn't carry those polygons.
///   3. **Kommune** (`/kommuneinfo/v1/punkt`) — the municipality at the
///      point, used as the final fallback.
///
/// Resolution priority:
///   1. A *tight* Stedsnavn hit ([LocationMatchTier.isTight]) wins
///      outright — pin sitting on a summit / inside a town should
///      always read that, even if a containing park is also returned.
///   2. Otherwise a containing protected area wins.
///   3. Otherwise a looser (periphery-tier) Stedsnavn hit wins.
///   4. Otherwise the kommune fallback.
///   5. `null` only when every source is empty (e.g. outside Norway).
class KartverketReverseGeocoder implements ReverseGeocoder {
  final StedsnavnBackend _stedsnavn;
  final ProtectedAreaBackend _protectedArea;
  final KommuneBackend _kommune;

  KartverketReverseGeocoder({
    required StedsnavnBackend stedsnavn,
    required ProtectedAreaBackend protectedArea,
    required KommuneBackend kommune,
  })  : _stedsnavn = stedsnavn,
        _protectedArea = protectedArea,
        _kommune = kommune;

  @override
  Future<LocationDescription?> describe(LatLng coord) async {
    // Fan-out the two parallel-safe lookups; await the cheaper one
    // (Stedsnavn) first and short-circuit if it's a tight hit.
    final stedsnavnFuture = _stedsnavn.find(coord);
    final vernFuture = _protectedArea.identifyAt(coord);

    final hit = await stedsnavnFuture;
    if (hit != null && hit.tier.isTight) {
      return hit.description;
    }

    final park = await vernFuture;
    if (park != null) return park;

    if (hit != null) return hit.description; // periphery-tier

    return _kommune.lookup(coord);
  }
}
