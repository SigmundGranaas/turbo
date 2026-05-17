import 'package:flutter/material.dart';
import 'package:flutter_map/flutter_map.dart';

enum TileProviderCategory { global, local, overlay, offline }

abstract class TileProviderConfig {
  String get id;
  String name(BuildContext context);
  String description(BuildContext context);
  String get attributions;
  String get urlTemplate;
  TileProviderCategory get category;

  // Optional configuration options
  double get minZoom => 1;
  double get maxZoom => 19;
  Map<String, String>? get headers => null;
  double get opacity => 1.0;

  /// When non-null, render this provider as a WMS layer via flutter_map's
  /// [WMSTileLayerOptions] instead of the standard `urlTemplate` path.
  /// Built-in providers leave this null; user-defined custom providers may
  /// supply parsed WMS options.
  WMSTileLayerOptions? get wmsOptions => null;
}