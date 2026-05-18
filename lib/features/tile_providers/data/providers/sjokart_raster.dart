import 'package:flutter/material.dart';
import 'package:turbo/core/util/user_agent.dart';
import 'package:turbo/features/tile_providers/models/tile_provider_config.dart';
import 'package:turbo/app/l10n/app_localizations.dart';

/// Kartverket's "sjokartraster" — the official Norwegian nautical chart with
/// depth contours, sounding marks, navigation aids, and shipping lanes. Sits
/// in the same WMTS namespace as `topo` so the URL pattern mirrors it.
///
/// Categorized as `local` since coverage is Norway-only; selecting it swaps
/// with the topo basemap so the user gets sea-chart context (depths, harbors)
/// instead of land topography.
class SjokartRasterConfig extends TileProviderConfig {
  @override
  String get id => 'sjokart';
  @override
  String name(BuildContext context) => context.l10n.layerNameSjokart;
  @override
  String description(BuildContext context) =>
      context.l10n.layerDescriptionSjokart;
  @override
  String get attributions => 'Kartverket sjøkart';
  @override
  TileProviderCategory get category => TileProviderCategory.local;
  @override
  String get urlTemplate =>
      'https://cache.atgcp1-prod.kartverket.cloud/v1/service?layer=sjokartraster&style=default&tilematrixset=webmercator&Service=WMTS&Request=GetTile&Version=1.0.0&Format=image/png&TileMatrix={z}&TileCol={x}&TileRow={y}';

  @override
  double get minZoom => 4.0;
  @override
  double get maxZoom => 18.0;
  @override
  Map<String, String>? get headers => {
    'User-Agent': kTurboUserAgent,
  };
}
