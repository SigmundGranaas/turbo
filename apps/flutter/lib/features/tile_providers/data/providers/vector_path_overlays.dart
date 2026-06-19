import 'package:flutter/material.dart';

import 'package:turbo/app/l10n/app_localizations.dart';
import 'package:turbo/features/tile_providers/models/tile_provider_config.dart';

/// Vector-only overlay configs — registry entries that exist purely so
/// the layer picker can toggle the corresponding [VectorDataLayer] in
/// `external_vector_layers/`. They ship no raster URL of their own;
/// `getActiveLayers` skips them.
///
/// IDs are kept stable so saved layer preferences keep working:
///   - `osm_paths`      → OSM Overpass paths
///   - `n50_sti`        → Kartverket N50 Sti/TraktorvegSti
abstract class _VectorPathToggleConfig extends TileProviderConfig {
  @override
  TileProviderCategory get category => TileProviderCategory.overlay;

  @override
  String get urlTemplate => '';

  @override
  bool get isVectorOnly => true;
}

class OsmPathsOverlayConfig extends _VectorPathToggleConfig {
  @override
  String get id => 'osm_paths';

  @override
  String name(BuildContext context) => context.l10n.layerNameOsmPaths;

  @override
  String description(BuildContext context) =>
      context.l10n.layerDescriptionOsmPaths;

  @override
  String get attributions => '© OpenStreetMap contributors';
}

class N50StiOverlayConfig extends _VectorPathToggleConfig {
  @override
  String get id => 'n50_sti';

  @override
  String name(BuildContext context) => context.l10n.layerNameN50Sti;

  @override
  String description(BuildContext context) =>
      context.l10n.layerDescriptionN50Sti;

  @override
  String get attributions => '© Kartverket — N50 Kartdata';
}

/// Toggle for the Nasjonal Turbase (ut.no / DNT) POI overlay — cabins and
/// trips fetched from `api.nasjonalturbase.no`, rendered as markers by
/// `features/nasjonal_turbase/` (NtbMarkerLayer + NtbRouteLayer). Vector-only:
/// `getActiveLayers` skips it; this entry just owns the on/off bit.
class NasjonalTurbasePoisOverlayConfig extends _VectorPathToggleConfig {
  @override
  String get id => 'nasjonal_turbase_pois';

  @override
  String name(BuildContext context) => context.l10n.layerNameNasjonalTurbase;

  @override
  String description(BuildContext context) =>
      context.l10n.layerDescriptionNasjonalTurbase;

  @override
  String get attributions => '© Nasjonal Turbase / DNT';
}

/// Curated MVT-served overlays from the self-hosted Turbo tileserver
/// (`apps/tileserver`). One toggle per resource exposed by `/v1/catalog`.
/// Geometry is rendered by `MvtDataLayer` from `features/curated_paths/`;
/// the registry just owns the on/off bit.
class CuratedHikingOverlayConfig extends _VectorPathToggleConfig {
  @override
  String get id => 'curated_hiking';

  @override
  String name(BuildContext context) => context.l10n.layerNameCuratedHiking;

  @override
  String description(BuildContext context) =>
      context.l10n.layerDescriptionCuratedHiking;

  @override
  String get attributions => '© Kartverket, Nasjonal Turbase';
}

class CuratedSkiTracksOverlayConfig extends _VectorPathToggleConfig {
  @override
  String get id => 'curated_ski_tracks';

  @override
  String name(BuildContext context) => context.l10n.layerNameCuratedSkiTracks;

  @override
  String description(BuildContext context) =>
      context.l10n.layerDescriptionCuratedSkiTracks;

  @override
  String get attributions => '© Kartverket, Skisporet.no';
}

class CuratedForestRoadsOverlayConfig extends _VectorPathToggleConfig {
  @override
  String get id => 'curated_forest_roads';

  @override
  String name(BuildContext context) => context.l10n.layerNameCuratedForestRoads;

  @override
  String description(BuildContext context) =>
      context.l10n.layerDescriptionCuratedForestRoads;

  @override
  String get attributions => '© Kartverket';
}

class CuratedCyclingRoutesOverlayConfig extends _VectorPathToggleConfig {
  @override
  String get id => 'curated_cycling_routes';

  @override
  String name(BuildContext context) =>
      context.l10n.layerNameCuratedCyclingRoutes;

  @override
  String description(BuildContext context) =>
      context.l10n.layerDescriptionCuratedCyclingRoutes;

  @override
  String get attributions => '© Kartverket';
}
