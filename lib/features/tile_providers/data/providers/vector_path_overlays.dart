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
