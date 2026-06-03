import 'package:flutter/widgets.dart';

import 'package:turbo/features/activities/api.dart' as activities;
import 'package:turbo/features/curated_paths/api.dart';
import 'package:turbo/features/external_vector_layers/api.dart';
import 'package:turbo/features/journey/api.dart';
import 'package:turbo/features/map_view/api.dart';
import 'package:turbo/features/path_recording/api.dart';
import 'package:turbo/features/photo_map/api.dart';
import 'package:turbo/features/saved_paths/api.dart';
import 'package:turbo/features/tile_providers/api.dart';
import 'package:turbo/features/weather/api.dart' show OceanConditionsLayer;

/// The default map-layer stack, composed at the app shell (which is allowed to
/// import features — like the activity-kind registry). Order is render order
/// and mirrors the stack `MainMapPage` used to hand-list. Base tiles +
/// attributions (host-owned) sit below this; the active tool's layers sit
/// above it. Adding a layer feature now means adding a descriptor here, not
/// editing `MainMapPage`.
MapLayerRegistry buildDefaultMapLayerRegistry() {
  return MapLayerRegistry([
    MapLayerDescriptor(
      id: 'recording_trace',
      build: (_) => const [RecordingTraceLayer()],
    ),
    MapLayerDescriptor(
      id: 'current_location',
      build: (_) => const [CurrentLocationLayer()],
    ),
    MapLayerDescriptor(
      id: 'journey_path',
      build: (_) => const [JourneyPathLayer()],
    ),
    MapLayerDescriptor(
      id: 'saved_paths',
      build: (ctx) => [SavedPathsLayer(mapController: ctx.mapController)],
    ),
    // Grouped trail/curated vector overlays (toggled via their overlay configs).
    MapLayerDescriptor(id: 'trail_overlays', build: _trailOverlays),
    MapLayerDescriptor(
      id: 'ocean_conditions',
      build: (ctx) => [
        OceanConditionsLayer(
          mapController: ctx.mapController,
          visible: _activeOverlayIds(ctx).contains('ocean_conditions'),
        ),
      ],
    ),
    MapLayerDescriptor(
      id: 'viewport_markers',
      build: (ctx) => [ViewportMarkers(mapController: ctx.mapController)],
    ),
    MapLayerDescriptor(
      id: 'activities',
      build: (_) => const [activities.ActivitiesMapLayer()],
    ),
    MapLayerDescriptor(
      id: 'photo_map',
      build: (ctx) => [PhotoMapLayer(mapController: ctx.mapController)],
    ),
  ]);
}

Set<String> _activeOverlayIds(MapLayerContext ctx) =>
    ctx.ref.watch(tileRegistryProvider).activeOverlayIds.toSet();

List<Widget> _trailOverlays(MapLayerContext ctx) {
  final activeOverlayIds = _activeOverlayIds(ctx);
  final mapController = ctx.mapController;
  return <Widget>[
    for (final entry in trailOverlayIdToSubtype.entries)
      VectorDataLayer(
        source: trailVectorSource(entry.value),
        mapController: mapController,
        visible: activeOverlayIds.contains(entry.key),
      ),
    VectorDataLayer(
      source: osmPathVectorSource(),
      mapController: mapController,
      visible: activeOverlayIds.contains('osm_paths'),
    ),
    VectorDataLayer(
      source: n50StiVectorSource(),
      mapController: mapController,
      visible: activeOverlayIds.contains('n50_sti'),
    ),
    for (final entry in ctx.ref.watch(curatedSourcesByIdProvider).entries)
      MvtDataLayer(
        source: entry.value,
        mapController: mapController,
        visible: activeOverlayIds.contains(entry.key),
      ),
  ];
}
