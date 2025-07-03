import 'package:flutter/material.dart';
import 'package:turbo/features/tile_providers/models/tile_provider_config.dart';
import 'package:turbo/l10n/app_localizations.dart';

class GoogleSatelliteConfig extends TileProviderConfig {
  @override
  String get id => 'gs';
  @override
  String name(BuildContext context) => context.l10n.layerNameGoogleSatellite;
  @override
  String description(BuildContext context) =>
      context.l10n.layerDescriptionGoogleSatellite;
  @override
  String get attributions => 'Google';
  @override
  TileProviderCategory get category => TileProviderCategory.global;
  @override
  String get urlTemplate =>
      'https://mt0.google.com/vt/lyrs=s&hl=en&x={x}&y={y}&z={z}';
  @override
  double get maxZoom => 20.0;
}