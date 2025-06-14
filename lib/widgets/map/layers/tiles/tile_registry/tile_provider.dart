import 'package:flutter/material.dart';
import 'package:flutter_map/flutter_map.dart';

abstract class TileProviderWrapper {
  String get id;
  String name(BuildContext context);
  String description(BuildContext context);
  String get attributions;

  TileCategory get category;

  TileLayer createTileLayer();

  // Optional configuration options
  double get minZoom => 1.0;
  double get maxZoom => 19.0;
  Map<String, String>? get headers => null;
}

enum TileCategory {
  global,
  local,
  overlay
}