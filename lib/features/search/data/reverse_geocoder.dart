import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:latlong2/latlong.dart';

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

/// Riverpod-managed singleton orchestrator. Returns the [ReverseGeocoder]
/// interface so consumers don't depend on the concrete Kartverket
/// implementation.
final reverseGeocoderProvider = Provider<ReverseGeocoder>((ref) {
  return KartverketReverseGeocoder(
    stedsnavn: ref.watch(stedsnavnBackendProvider),
    protectedArea: ref.watch(protectedAreaBackendProvider),
    kommune: ref.watch(kommuneBackendProvider),
  );
});

/// Cache key for [describeLocationProvider]: quantises a [LatLng] to a
/// ~500 m grid so taps on the same area share the cached lookup.
class GeoQuery {
  /// Quantised coordinate (~500 m grid).
  final LatLng coord;

  GeoQuery(LatLng input)
      : coord = LatLng(
          _q(input.latitude),
          _q(input.longitude),
        );

  static double _q(double v) => (v * 200).round() / 200.0;

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
/// of re-firing HTTP.
final describeLocationProvider =
    FutureProvider.family<LocationDescription?, GeoQuery>((ref, query) async {
  return ref.watch(reverseGeocoderProvider).describe(query.coord);
});
