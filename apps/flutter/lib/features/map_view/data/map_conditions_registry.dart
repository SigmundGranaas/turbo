import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../models/conditions_source.dart';

/// Registry of conditions sources. Build-specific (which forecast features
/// ship), so the default is empty and `app/main.dart` overrides it.
class MapConditionsRegistry {
  final List<ConditionsSource> sources;

  MapConditionsRegistry(Iterable<ConditionsSource> sources)
      : sources = sources.toList(growable: false);

  bool get isNotEmpty => sources.isNotEmpty;
}

final mapConditionsRegistryProvider = Provider<MapConditionsRegistry>((ref) {
  return MapConditionsRegistry(const []);
});
