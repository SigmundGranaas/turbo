import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:latlong2/latlong.dart';

import 'backends/address_backend.dart';
import 'backends/elevation_backend.dart';
import 'backends/kommune_backend.dart';
import 'backends/protected_area_backend.dart';
import 'backends/stedsnavn_backend.dart';
import 'kartverket_reverse_geocoder.dart';
import 'location_service.dart';

/// Reverse-geocodes a coordinate to a contextual [LocationDescription]
/// (e.g. "On Galdhøpiggen" / "In Lom" / "In Saltfjellet–Svartisen
/// nasjonalpark"). Returns `null` when no source produced a usable
/// label — UI is expected to fall back to raw coordinates.
abstract class ReverseGeocoder {
  Future<LocationDescription?> describe(LatLng coord);
}

/// Backend providers — overridable in tests for fine-grained mocking.
final stedsnavnBackendProvider = Provider<StedsnavnBackend>(
  (ref) => StedsnavnBackend(),
);

final protectedAreaBackendProvider = Provider<ProtectedAreaBackend>(
  (ref) => ProtectedAreaBackend(),
);

final kommuneBackendProvider = Provider<KommuneBackend>(
  (ref) => KommuneBackend(),
);

final addressBackendProvider = Provider<AddressBackend>(
  (ref) => AddressBackend(),
);

final elevationBackendProvider = Provider<ElevationBackend>(
  (ref) => ElevationBackend(),
);

/// Riverpod-managed singleton orchestrator. Returns the [ReverseGeocoder]
/// interface so consumers don't depend on the concrete Kartverket
/// implementation.
final reverseGeocoderProvider = Provider<ReverseGeocoder>((ref) {
  return KartverketReverseGeocoder(
    stedsnavn: ref.watch(stedsnavnBackendProvider),
    protectedArea: ref.watch(protectedAreaBackendProvider),
    kommune: ref.watch(kommuneBackendProvider),
    address: ref.watch(addressBackendProvider),
    elevation: ref.watch(elevationBackendProvider),
  );
});

/// Cache key for [describeLocationProvider]: quantises a [LatLng] to a
/// ~250 m grid so taps on the same area share the cached lookup. 500 m
/// was wide enough to alias adjacent peaks (Galdhøpiggen / Glittertind
/// share a grid cell at that resolution); 250 m keeps the cache busy
/// while still respecting feature granularity.
class GeoQuery {
  /// Quantised coordinate (~250 m grid).
  final LatLng coord;

  GeoQuery(LatLng input)
      : coord = LatLng(
          _q(input.latitude),
          _q(input.longitude),
        );

  // 0.0025° latitude ≈ 277 m. Same step on longitude ≈ 130–170 m
  // at Nordic latitudes — close enough to "~250 m" for cache purposes.
  static double _q(double v) => (v * 400).round() / 400.0;

  @override
  bool operator ==(Object other) =>
      other is GeoQuery &&
      other.coord.latitude == coord.latitude &&
      other.coord.longitude == coord.longitude;

  @override
  int get hashCode => Object.hash(coord.latitude, coord.longitude);
}

/// Family-cached reverse-geocode: a `ref.watch(describeLocationProvider(
/// GeoQuery(point)))` on the same area reuses the prior result instead
/// of re-firing HTTP. `keepAlive` is explicit so the cache survives the
/// sheet being closed and reopened.
final describeLocationProvider =
    FutureProvider.family<LocationDescription?, GeoQuery>((ref, query) async {
  ref.keepAlive();
  return ref.watch(reverseGeocoderProvider).describe(query.coord);
});
