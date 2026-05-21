import 'package:flutter/material.dart';

enum TileProviderCategory { global, local, overlay, offline }

abstract class TileProviderConfig {
  String get id;
  String name(BuildContext context);
  String description(BuildContext context);
  String get attributions;
  String get urlTemplate;
  TileProviderCategory get category;

  // Optional configuration options
  double get minZoom => 1.0;
  double get maxZoom => 19.0;
  Map<String, String>? get headers => null;
  double get opacity => 1.0;
}