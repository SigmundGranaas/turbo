import 'package:flutter/material.dart';
import 'package:turbo/features/tile_providers/models/tile_provider_config.dart';
import 'package:turbo/l10n/app_localizations.dart';

class NorgeskartTopoConfig extends TileProviderConfig {
  @override
  String get id => 'topo';
  @override
  String name(BuildContext context) => context.l10n.layerNameNorgeskart;
  @override
  String description(BuildContext context) =>
      context.l10n.layerDescriptionNorgeskart;
  @override
  String get attributions => 'Norgeskart';
  @override
  TileProviderCategory get category => TileProviderCategory.local;
  @override
  String get urlTemplate =>
      'https://cache.atgcp1-prod.kartverket.cloud/v1/service?layer=topo&style=default&tilematrixset=webmercator&Service=WMTS&Request=GetTile&Version=1.0.0&Format=image/png&TileMatrix={z}&TileCol={x}&TileRow={y}';

  @override
  double get minZoom => 4.0;
  @override
  double get maxZoom => 20.0;
}