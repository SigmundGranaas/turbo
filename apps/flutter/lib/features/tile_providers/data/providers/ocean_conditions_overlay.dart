import 'package:flutter/material.dart';

import 'package:turbo/app/l10n/app_localizations.dart';
import 'package:turbo/features/tile_providers/models/tile_provider_config.dart';

/// Vector-only overlay toggle for the live ocean-conditions layer.
///
/// Like the OSM/N50 path overlays, this config ships no raster tiles — it
/// only owns the on/off bit in the registry. The actual geometry (wave
/// height + direction sampled across the viewport) is drawn by
/// `OceanConditionsLayer` in the weather feature, which reads MET Norway's
/// `oceanforecast/2.0` endpoint — the same data source that powers the
/// weather sheet's "Sea conditions" tab.
class OceanConditionsOverlayConfig extends TileProviderConfig {
  @override
  String get id => 'ocean_conditions';

  @override
  String name(BuildContext context) => context.l10n.layerNameOceanConditions;

  @override
  String description(BuildContext context) =>
      context.l10n.layerDescriptionOceanConditions;

  @override
  String get attributions => 'MET Norway';

  @override
  TileProviderCategory get category => TileProviderCategory.overlay;

  @override
  String get urlTemplate => '';

  @override
  bool get isVectorOnly => true;
}
