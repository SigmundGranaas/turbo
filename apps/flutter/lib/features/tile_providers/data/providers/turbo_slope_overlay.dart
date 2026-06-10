import 'package:flutter/material.dart';
import 'package:turbo/core/util/user_agent.dart';
import 'package:turbo/features/tile_providers/models/tile_provider_config.dart';

/// Slope-angle ("bratthet") overlay from our own tileserver
/// (`/v1/slope/tiles/{z}/{x}/{y}.png`), derived from the Kartverket DEM.
/// Varsom-convention avalanche bands (27–30–35–40–45°), transparent below
/// 27°. Self-hosted alternative to the NVE steepness WMTS — selecting it
/// instead of the NVE layer removes the `gis3.nve.no` dependency. (NVE's
/// "med utløp" runout zones are NOT included — this is pure slope angle.)
class TurboSlopeOverlayConfig extends TileProviderConfig {
  TurboSlopeOverlayConfig(this._baseUrl);

  /// Tileserver base URL, resolved by the registry from
  /// `tileserverBaseUrlProvider` (curated_paths api), same as the N50 layers.
  final String _baseUrl;

  @override
  String get id => 'turbo_slope';
  @override
  String name(BuildContext context) => 'Bratthet (Turbo)';
  @override
  String description(BuildContext context) =>
      'Helningsgrader fra egen høydemodell: 27–30–35–40–45°. Uten utløpssoner.';
  @override
  String get attributions => '© Kartverket (DTM)';
  @override
  TileProviderCategory get category => TileProviderCategory.overlay;
  @override
  String get urlTemplate => '$_baseUrl/v1/slope/tiles/{z}/{x}/{y}.png';

  @override
  double get minZoom => 8.0;
  @override
  double get maxZoom => 15.0; // DTM10-scale data; overzoom beyond
  @override
  double get opacity => 0.8;
  @override
  Map<String, String>? get headers => {
    'User-Agent': kTurboUserAgent,
  };
}
