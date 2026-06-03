import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../models/map_selection.dart';

/// Holds the currently-inspected map entity (or null). Any surface selects via
/// [select]; the [MapEntityDetailHost] overlay watches this and presents the
/// detail sheet. One seam so taps, long-presses and search results all funnel
/// through the same detail + action bar.
class SelectedMapEntityNotifier extends Notifier<MapSelection?> {
  @override
  MapSelection? build() => null;

  void select(MapSelection selection) => state = selection;

  void clear() => state = null;
}

final selectedMapEntityProvider =
    NotifierProvider<SelectedMapEntityNotifier, MapSelection?>(
  SelectedMapEntityNotifier.new,
);
