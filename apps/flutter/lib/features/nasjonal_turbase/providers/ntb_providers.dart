import 'package:flutter_map/flutter_map.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import 'package:turbo/core/config/env_config.dart';
import '../data/ntb_client.dart';
import '../data/ntb_repository.dart';
import '../models/ntb_poi.dart';
import '../models/ntb_route.dart';

/// Registry id of the toggleable NTB overlay (matches the tile-registry config
/// and the layer-picker entry).
const String ntbOverlayId = 'nasjonal_turbase_pois';

/// Below this zoom the viewport is too wide to usefully query/scatter markers.
const double ntbMinZoom = 9.0;

/// Backend base URL (the Turbo API that hosts the NTB proxy), overridable in
/// tests.
final ntbBaseUrlProvider = Provider<String>((_) => EnvironmentConfig.apiBaseUrl);

final ntbClientProvider = Provider<NtbClient>(
  (ref) => NtbClient(baseUrl: ref.watch(ntbBaseUrlProvider)),
);

final ntbRepositoryProvider = Provider<NtbRepository>(
  (ref) => NtbRepository(client: ref.watch(ntbClientProvider)),
);

/// The POIs currently loaded for the viewport. Refreshed by the marker layer on
/// map-movement-end; guarded by a sequence counter so a slow fetch can't clobber
/// a newer one.
final ntbViewportPoisProvider =
    NotifierProvider<NtbViewportPois, List<NtbPoi>>(NtbViewportPois.new);

class NtbViewportPois extends Notifier<List<NtbPoi>> {
  int _seq = 0;

  @override
  List<NtbPoi> build() => const [];

  Future<void> load(LatLngBounds bounds, double zoom) async {
    if (zoom < ntbMinZoom) {
      if (state.isNotEmpty) state = const [];
      return;
    }
    final seq = ++_seq;
    final pois = await ref.read(ntbRepositoryProvider).poisInBounds(
          minLat: bounds.south,
          minLon: bounds.west,
          maxLat: bounds.north,
          maxLon: bounds.east,
        );
    if (seq != _seq) return; // a newer load superseded this one
    state = pois;
  }
}

/// The trip whose route is being presented. [token] increments on every new
/// selection so the route layer restarts its reveal animation even when the
/// same trip is re-selected.
class NtbRouteSelection {
  final NtbPoi poi;
  final NtbRoute? route; // null while the route is still loading
  final int token;

  const NtbRouteSelection({
    required this.poi,
    required this.route,
    required this.token,
  });

  NtbRouteSelection copyWith({NtbRoute? route}) => NtbRouteSelection(
        poi: poi,
        route: route ?? this.route,
        token: token,
      );
}

final ntbSelectedRouteProvider =
    NotifierProvider<NtbSelectedRoute, NtbRouteSelection?>(
        NtbSelectedRoute.new);

class NtbSelectedRoute extends Notifier<NtbRouteSelection?> {
  int _token = 0;

  @override
  NtbRouteSelection? build() => null;

  /// Selects [poi] and (for trips) fetches its route, then publishes it so the
  /// route layer animates the reveal. Non-trip POIs just clear any route.
  Future<void> select(NtbPoi poi) async {
    if (!poi.hasRoute) {
      clear();
      return;
    }
    final token = ++_token;
    state = NtbRouteSelection(poi: poi, route: null, token: token);
    final route = await ref.read(ntbRepositoryProvider).route(poi.id);
    if (token != _token) return; // superseded
    state = NtbRouteSelection(poi: poi, route: route, token: token);
  }

  void clear() {
    _token++;
    state = null;
  }
}
