import 'package:flutter/material.dart';
import 'package:turbo/features/tile_providers/models/tile_provider_config.dart';
import 'package:turbo/l10n/app_localizations.dart';

class OsmConfig extends TileProviderConfig {
  @override
  String get id => 'osm';
  @override
  String name(BuildContext context) => context.l10n.layerNameOsm;
  @override
  String description(BuildContext context) => context.l10n.layerDescriptionOsm;
  @override
  String get attributions => 'OpenStreetMap contributors';
  @override
  TileProviderCategory get category => TileProviderCategory.global;
  @override
  String get urlTemplate => 'https://tile.openstreetmap.org/{z}/{x}/{y}.png';
  @override
  double get maxZoom => 19.0;
  @override
  Map<String, String>? get headers => {
    'User-Agent': 'turbo_map_app/1.0.18 (+https://github.com/sigmundgranaas/turbo)',
  };
}