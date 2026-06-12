import 'package:flutter/material.dart';
import 'package:turbo/core/util/user_agent.dart';
import 'package:turbo/features/tile_providers/models/tile_provider_config.dart';

/// Self-hosted N50 topo raster basemap from our own tileserver
/// (`/v1/raster/n50/{z}/{x}/{y}.png`) — the same PostGIS data and the same
/// `n50-topo` style as the vector basemap, rasterised at the origin and
/// cached by the edge worker. Selecting this layer instead of Norgeskart
/// removes the external Kartverket WMTS dependency (M1 of the basemap
/// plan). Tiles end at z16 (N50 scale); the registry overzooms above that.
class TurboN50TopoConfig extends TileProviderConfig {
  TurboN50TopoConfig(this._baseUrl);

  /// Tileserver base URL, resolved by the registry from
  /// `tileserverBaseUrlProvider` (curated_paths api) so dev/staging/prod
  /// follow the same `--dart-define=TURBO_TILESERVER_URL` override.
  final String _baseUrl;

  @override
  String get id => 'turbo_n50_topo';
  @override
  String name(BuildContext context) => 'N50 topo (Turbo)';
  @override
  String description(BuildContext context) =>
      'Selvhostet norsk topokart fra åpne Kartverket N50-data.';
  @override
  String get attributions => '© Kartverket';
  @override
  TileProviderCategory get category => TileProviderCategory.local;
  @override
  String get urlTemplate => '$_baseUrl/v1/raster/n50/{z}/{x}/{y}.png';

  @override
  double get minZoom => 4.0;
  @override
  double get maxZoom => 16.0; // native max; flutter_map overzooms beyond
  @override
  Map<String, String>? get headers => {
    'User-Agent': kTurboUserAgent,
  };
}
