import 'package:flutter/material.dart';
import 'package:turbo/core/util/user_agent.dart';
import 'package:turbo/features/tile_providers/models/tile_provider_config.dart';
import 'package:turbo/app/l10n/app_localizations.dart';

/// OpenSeaMap seamark overlay — buoys, lights, harbors, depth points, and
/// other nautical symbols rendered on top of any base layer. The tile set is
/// transparent everywhere except where seamarks exist, so it composes with
/// land basemaps cleanly.
class OpenSeaMapOverlayConfig extends TileProviderConfig {
  @override
  String get id => 'openseamap';
  @override
  String name(BuildContext context) => context.l10n.layerNameSeamarks;
  @override
  String description(BuildContext context) =>
      context.l10n.layerDescriptionSeamarks;
  @override
  String get attributions => 'OpenSeaMap contributors';
  @override
  TileProviderCategory get category => TileProviderCategory.overlay;
  @override
  String get urlTemplate =>
      'https://tiles.openseamap.org/seamark/{z}/{x}/{y}.png';
  @override
  double get maxZoom => 18.0;
  @override
  Map<String, String>? get headers => {
    'User-Agent': kTurboUserAgent,
  };
}
