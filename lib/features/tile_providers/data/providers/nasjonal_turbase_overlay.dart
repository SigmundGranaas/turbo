import 'package:flutter/material.dart';
import 'package:flutter_map/flutter_map.dart';
import 'package:turbo/core/util/user_agent.dart';
import 'package:turbo/features/tile_providers/models/tile_provider_config.dart';
import 'package:turbo/app/l10n/app_localizations.dart';

/// Nasjonal turbase trail-network overlay served by Geonorge / Kartverket as
/// a WMS feed. Toggling this overlay also activates the vector counterpart
/// that handles tap-to-inspect (wired separately in the map page).
class NasjonalTurbaseOverlayConfig extends TileProviderConfig {
  /// Geonorge WMS for "Tur- og friluftsruter" (national trail registry).
  /// Uses the `?` separator at the end so flutter_map can append the WMS
  /// query parameters cleanly.
  static const String _wmsUrl =
      'https://wms.geonorge.no/skwms1/wms.friluftsruter2';

  @override
  String get id => 'nasjonal_turbase';

  @override
  String name(BuildContext context) => context.l10n.layerNameTrails;

  @override
  String description(BuildContext context) =>
      context.l10n.layerDescriptionTrails;

  @override
  String get attributions => 'Kartverket / DNT';

  @override
  TileProviderCategory get category => TileProviderCategory.overlay;

  @override
  String get urlTemplate => _wmsUrl;

  @override
  double get opacity => 0.9;

  /// WMS overlays are only useful above a certain zoom — at low zooms the
  /// raster tiles are mostly empty. 7 keeps the request count low while
  /// covering normal in-app exploration.
  @override
  double get minZoom => 7;

  @override
  Map<String, String>? get headers => {
        'User-Agent': kTurboUserAgent,
      };

  @override
  WMSTileLayerOptions? get wmsOptions => WMSTileLayerOptions(
        baseUrl: '$_wmsUrl?',
        // The published layer names in Geonorge's friluftsruter2 capabilities
        // document. Using WMS 1.1.1 — flutter_map issues 1.3.0 requests by
        // default but Geonorge's older WMS instance is more reliable on 1.1.1.
        layers: const ['fotrute', 'skiloype', 'andreruter', 'sykkelrute'],
        format: 'image/png',
        transparent: true,
        version: '1.1.1',
      );
}
