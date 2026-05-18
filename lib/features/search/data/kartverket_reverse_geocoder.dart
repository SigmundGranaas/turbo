import 'package:latlong2/latlong.dart';

import 'backends/address_backend.dart';
import 'backends/elevation_backend.dart';
import 'backends/kommune_backend.dart';
import 'backends/protected_area_backend.dart';
import 'backends/stedsnavn_backend.dart';
import 'location_service.dart';
import 'reverse_geocoder.dart';

/// Default Norway-first [ReverseGeocoder]. Composes five independent
/// backends:
///
///   1. **Stedsnavn** (`/stedsnavn/v1/punkt`) — toponyms: peaks,
///      settlements, water, farms. Scored by tier (`exactContact` /
///      `inSettlement` / `closeToPeak` / `periphery`).
///   2. **Protected area** (Miljødirektoratet Vern Identify) — national
///      parks, nature reserves, landscape-protected areas. Required for
///      "In Saltfjellet–Svartisen nasjonalpark" to ever resolve, since
///      Stedsnavn doesn't carry those polygons.
///   3. **Address** (`/adresser/v1/punktsok`) — nearest civic address.
///      Slotted between protected-area and kommune so populated rural
///      pins read "Near Storgården 4, Lom" instead of bare kommune.
///   4. **Kommune** (`/kommuneinfo/v1/punkt`) — the municipality at the
///      point, used as the final fallback.
///   5. **Elevation** (`/hoydedata/v1/punkt`) — metres above sea level.
///      Fired in parallel; the result is merged into whichever
///      description wins ("Galdhøpiggen, 2469 m").
///
/// Resolution priority:
///   1. A *tight* Stedsnavn hit ([LocationMatchTier.isTight]) wins
///      outright — pin sitting on a summit / inside a town should
///      always read that, even if a containing park is also returned.
///   2. Otherwise a containing protected area wins.
///   3. Otherwise a looser (periphery-tier) Stedsnavn hit wins.
///   4. Otherwise the nearest address wins.
///   5. Otherwise the kommune fallback.
///   6. `null` only when every source is empty (e.g. outside Norway).
class KartverketReverseGeocoder implements ReverseGeocoder {
  final StedsnavnBackend _stedsnavn;
  final ProtectedAreaBackend _protectedArea;
  final KommuneBackend _kommune;
  final AddressBackend? _address;
  final ElevationBackend? _elevation;

  KartverketReverseGeocoder({
    required StedsnavnBackend stedsnavn,
    required ProtectedAreaBackend protectedArea,
    required KommuneBackend kommune,
    AddressBackend? address,
    ElevationBackend? elevation,
  })  : _stedsnavn = stedsnavn,
        _protectedArea = protectedArea,
        _kommune = kommune,
        _address = address,
        _elevation = elevation;

  @override
  Future<LocationDescription?> describe(LatLng coord) async {
    // Fan-out the network-bound sources. Elevation is an enrichment
    // that's merged into whichever description wins.
    final stedsnavnFuture = _stedsnavn.find(coord);
    final vernFuture = _protectedArea.identifyAt(coord);
    final addressFuture = _address?.nearestAddress(coord);
    final elevationFuture = _elevation?.elevationAt(coord);

    Future<LocationDescription?> enrich(LocationDescription? d) async {
      if (d == null) return null;
      final elev = await elevationFuture;
      return elev == null ? d : d.copyWith(elevationMeters: elev);
    }

    final hit = await stedsnavnFuture;
    if (hit != null && hit.tier.isTight) {
      return enrich(hit.description);
    }

    final park = await vernFuture;
    if (park != null) return enrich(park);

    if (hit != null) return enrich(hit.description); // periphery-tier

    final address = await (addressFuture ?? Future.value(null));
    if (address != null) return enrich(address);

    return enrich(await _kommune.lookup(coord));
  }
}
