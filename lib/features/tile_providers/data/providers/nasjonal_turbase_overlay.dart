import 'package:flutter/material.dart';
import 'package:flutter_map/flutter_map.dart';
import 'package:turbo/core/util/user_agent.dart';
import 'package:turbo/features/tile_providers/models/tile_provider_config.dart';
import 'package:turbo/app/l10n/app_localizations.dart';

/// Base for the four Nasjonal turbase trail-subtype overlays. Each subclass
/// exposes a single Geonorge WMS layer name (footpath / ski track / bike
/// route / other) so the user can toggle each subtype independently.
abstract class _TrailsOverlayConfig extends TileProviderConfig {
  static const String _wmsUrl =
      'https://wms.geonorge.no/skwms1/wms.friluftsruter2';

  String get wmsLayer;

  @override
  String get attributions => 'Kartverket / DNT';

  @override
  TileProviderCategory get category => TileProviderCategory.overlay;

  @override
  String get urlTemplate => _wmsUrl;

  @override
  double get opacity => 0.9;

  @override
  double get minZoom => 7;

  @override
  Map<String, String>? get headers => {
        'User-Agent': kTurboUserAgent,
      };

  @override
  WMSTileLayerOptions? get wmsOptions => WMSTileLayerOptions(
        baseUrl: '$_wmsUrl?',
        layers: [wmsLayer],
        format: 'image/png',
        transparent: true,
        version: '1.1.1',
      );
}

class TrailsFootOverlayConfig extends _TrailsOverlayConfig {
  @override
  String get id => 'trails_foot';
  @override
  String get wmsLayer => 'fotrute';
  @override
  String name(BuildContext context) => context.l10n.layerNameTrailsFoot;
  @override
  String description(BuildContext context) =>
      context.l10n.layerDescriptionTrailsFoot;
}

class TrailsSkiOverlayConfig extends _TrailsOverlayConfig {
  @override
  String get id => 'trails_ski';
  @override
  String get wmsLayer => 'skiloype';
  @override
  String name(BuildContext context) => context.l10n.layerNameTrailsSki;
  @override
  String description(BuildContext context) =>
      context.l10n.layerDescriptionTrailsSki;
}

class TrailsBikeOverlayConfig extends _TrailsOverlayConfig {
  @override
  String get id => 'trails_bike';
  @override
  String get wmsLayer => 'sykkelrute';
  @override
  String name(BuildContext context) => context.l10n.layerNameTrailsBike;
  @override
  String description(BuildContext context) =>
      context.l10n.layerDescriptionTrailsBike;
}

class TrailsOtherOverlayConfig extends _TrailsOverlayConfig {
  @override
  String get id => 'trails_other';
  @override
  String get wmsLayer => 'andreruter';
  @override
  String name(BuildContext context) => context.l10n.layerNameTrailsOther;
  @override
  String description(BuildContext context) =>
      context.l10n.layerDescriptionTrailsOther;
}
