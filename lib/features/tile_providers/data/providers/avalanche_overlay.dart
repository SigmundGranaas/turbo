import 'package:flutter/material.dart';
import 'package:turbo/features/tile_providers/models/tile_provider_config.dart';
import 'package:turbo/l10n/app_localizations.dart';

class AvalancheOverlayConfig extends TileProviderConfig {
  @override
  String get id => 'avalanche_danger';
  @override
  String name(BuildContext context) => context.l10n.layerNameAvalanche;
  @override
  String description(BuildContext context) =>
      context.l10n.layerDescriptionAvalanche;
  @override
  String get attributions => 'NVE';
  @override
  TileProviderCategory get category => TileProviderCategory.overlay;
  @override
  String get urlTemplate =>
      'https://gis3.nve.no/arcgis/rest/services/wmts/Bratthet_med_utlop_2024/MapServer/tile/{z}/{y}/{x}';
  @override
  double get opacity => 0.7;
  @override
  Map<String, String>? get headers => {
    'User-Agent': 'turbo_map_app/1.0.18 (+https://github.com/sigmundgranaas/turbo)',
  };
}