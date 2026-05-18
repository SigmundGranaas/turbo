import 'package:flutter/material.dart';
import 'package:flutter_map/flutter_map.dart';
import 'package:turbo/core/util/user_agent.dart';
import 'package:turbo/features/tile_providers/models/tile_provider_config.dart';
import 'package:turbo/app/l10n/app_localizations.dart';

/// Nasjonal turbase trail-network overlay, served by Kartverket / Geonorge
/// as a WMS feed. The vector counterpart for tap-to-inspect lives in the
/// external_vector_layers feature.
class NasjonalTurbaseOverlayConfig extends TileProviderConfig {
  static const String _wmsUrl =
      'https://openwms.statkart.no/skwms1/wms.friluftsruter2';

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
  double get opacity => 0.85;

  @override
  Map<String, String>? get headers => {
        'User-Agent': kTurboUserAgent,
      };

  @override
  WMSTileLayerOptions? get wmsOptions => WMSTileLayerOptions(
        baseUrl: '$_wmsUrl?',
        // "Fotrute" = footpath, "Skiloype" = ski track, "Annenrute" = other.
        // Pull all three so the user sees the complete official network.
        layers: const ['Fotrute', 'Skiloype', 'Annenrute'],
        format: 'image/png',
        transparent: true,
        version: '1.3.0',
      );
}
