import 'package:flutter_map/flutter_map.dart';
import 'package:map_app/widgets/map/layers/tiles/providers/tile_provider.dart';

class MockTileProviderWrapper implements TileProviderWrapper {
  @override
  final String id;

  @override
  final String name;

  @override
  final String description;

  @override
  final TileCategory category;

  MockTileProviderWrapper({
    required this.id,
    this.name = 'Mock Provider',
    this.description = 'Mock Description',
    required this.category,
  });

  @override
  TileLayer createTileLayer() {
    return TileLayer(
      urlTemplate: 'https://mock.test/{z}/{x}/{y}.png',
      subdomains: const ['a', 'b', 'c'],
      minZoom: minZoom,
      maxZoom: maxZoom,
    );
  }

  @override
  double get opacity => 1.0;

  @override
  double get minZoom => 1.0;

  @override
  double get maxZoom => 19.0;

  @override
  Map<String, String>? get headers => null;
}