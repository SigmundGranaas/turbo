import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import 'package:turbo/app/l10n/app_localizations.dart';
import '../models/mvt_layer_source.dart';

/// Base URL of the curated-paths tileserver (apps/tileserver). Lives in
/// a provider so tests can override it and CI/local can point at a
/// staging deploy via `--dart-define`.
final tileserverBaseUrlProvider = Provider<String>((ref) {
  // `--dart-define=TURBO_TILESERVER_URL=https://tiles.example.com`
  const fromEnv = String.fromEnvironment('TURBO_TILESERVER_URL');
  if (fromEnv.isNotEmpty) return fromEnv;
  // Sensible defaults: localhost for dev (compose maps 8090), the
  // gateway-prefixed prod URL for everything else.
  if (kDebugMode) return 'http://localhost:8090';
  return 'https://api.sandring.no/api/tiles';
});

/// One [MvtLayerSource] per curated resource, named to match the
/// registry IDs in `vector_path_overlays.dart`.
final curatedHikingSourceProvider = Provider<MvtLayerSource>((ref) {
  final base = ref.watch(tileserverBaseUrlProvider);
  return MvtLayerSource(
    id: 'curated_hiking',
    name: (c) => c.l10n.layerNameCuratedHiking,
    tilesUrlTemplate: '$base/v1/hiking-trails/tiles/{z}/{x}/{y}.mvt',
    geojsonDetailUrlTemplate: '$base/v1/hiking-trails/{id}',
    color: const Color(0xFFE53935),
    attribution: '© Kartverket, Nasjonal Turbase',
  );
});

final curatedSkiTracksSourceProvider = Provider<MvtLayerSource>((ref) {
  final base = ref.watch(tileserverBaseUrlProvider);
  return MvtLayerSource(
    id: 'curated_ski_tracks',
    name: (c) => c.l10n.layerNameCuratedSkiTracks,
    tilesUrlTemplate: '$base/v1/ski-tracks/tiles/{z}/{x}/{y}.mvt',
    geojsonDetailUrlTemplate: '$base/v1/ski-tracks/{id}',
    color: const Color(0xFF1E88E5),
    attribution: '© Kartverket, Skisporet.no',
  );
});

final curatedForestRoadsSourceProvider = Provider<MvtLayerSource>((ref) {
  final base = ref.watch(tileserverBaseUrlProvider);
  return MvtLayerSource(
    id: 'curated_forest_roads',
    name: (c) => c.l10n.layerNameCuratedForestRoads,
    tilesUrlTemplate: '$base/v1/forest-roads/tiles/{z}/{x}/{y}.mvt',
    geojsonDetailUrlTemplate: '$base/v1/forest-roads/{id}',
    color: const Color(0xFF6D4C41),
    attribution: '© Kartverket',
  );
});

final curatedCyclingRoutesSourceProvider = Provider<MvtLayerSource>((ref) {
  final base = ref.watch(tileserverBaseUrlProvider);
  return MvtLayerSource(
    id: 'curated_cycling_routes',
    name: (c) => c.l10n.layerNameCuratedCyclingRoutes,
    tilesUrlTemplate: '$base/v1/cycling-routes/tiles/{z}/{x}/{y}.mvt',
    geojsonDetailUrlTemplate: '$base/v1/cycling-routes/{id}',
    color: const Color(0xFF43A047),
    attribution: '© Kartverket',
  );
});

/// All curated sources keyed by registry id, for the map-page wiring
/// to look up which [MvtLayerSource] to feed into an [MvtDataLayer]
/// based on the active overlay ids.
final curatedSourcesByIdProvider = Provider<Map<String, MvtLayerSource>>((ref) {
  return {
    for (final p in [
      ref.watch(curatedHikingSourceProvider),
      ref.watch(curatedSkiTracksSourceProvider),
      ref.watch(curatedForestRoadsSourceProvider),
      ref.watch(curatedCyclingRoutesSourceProvider),
    ])
      p.id: p,
  };
});
