import 'package:flutter/material.dart';
import 'package:intl/intl.dart';
import 'package:turbo/features/tile_providers/models/tile_provider_config.dart';
import 'package:turbo/features/tile_storage/offline_regions/api.dart';

/// An adapter that makes an [OfflineRegion] compatible with the
/// [TileProviderConfig] interface, allowing it to be managed by the TileRegistry.
class OfflineRegionProviderConfig extends TileProviderConfig {
  final OfflineRegion region;

  OfflineRegionProviderConfig(this.region);

  @override
  String get id => region.id;

  @override
  String name(BuildContext context) => region.name;

  @override
  String description(BuildContext context) {
    final formattedDate = DateFormat.yMMMd().format(region.createdAt);
    return "Downloaded on $formattedDate";
  }

  @override
  String get attributions => region.tileProviderName;

  @override
  TileProviderCategory get category => TileProviderCategory.offline;

  @override
  String get urlTemplate => region.urlTemplate;

  @override
  double get minZoom => region.minZoom.toDouble();

  @override
  double get maxZoom => region.maxZoom.toDouble();
}