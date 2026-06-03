import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import 'package:turbo/core/widgets/exclusive_sheet.dart';

import '../data/map_selection_notifier.dart';
import '../models/map_selection.dart';
import 'map_entity_detail_sheet.dart';

/// Zero-footprint overlay that presents the detail sheet whenever something is
/// selected on the map. Lives in the overlay stack (always mounted), so any
/// surface can drive the detail sheet by calling
/// `selectedMapEntityProvider.notifier.select(...)` — taps, long-presses and
/// search results all funnel through this one host instead of each opening
/// their own `showExclusiveSheet`.
class MapEntityDetailHost extends ConsumerStatefulWidget {
  const MapEntityDetailHost({super.key});

  @override
  ConsumerState<MapEntityDetailHost> createState() =>
      _MapEntityDetailHostState();
}

class _MapEntityDetailHostState extends ConsumerState<MapEntityDetailHost> {
  bool _presenting = false;

  @override
  Widget build(BuildContext context) {
    ref.listen<MapSelection?>(selectedMapEntityProvider, (prev, next) {
      if (next != null && !_presenting) _present(next);
    });
    return const SizedBox.shrink();
  }

  Future<void> _present(MapSelection selection) async {
    _presenting = true;
    try {
      await showExclusiveSheet<void>(
        context,
        backgroundColor: Colors.transparent,
        builder: (_) => MapEntityDetailSheet(selection: selection),
      );
    } finally {
      _presenting = false;
      // The sheet closed (by action, close button, or scrim tap) — drop the
      // selection so the next select() re-presents and consumers see "nothing
      // selected".
      if (mounted) ref.read(selectedMapEntityProvider.notifier).clear();
    }
  }
}
