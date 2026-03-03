import 'package:flutter/material.dart';
import 'package:flutter_map/flutter_map.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import 'package:turbo/core/location/location_state.dart';
import 'package:turbo/features/navigation/data/navigation_state_notifier.dart';

class NavigationPolylineLayer extends ConsumerWidget {
  const NavigationPolylineLayer({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final navState = ref.watch(navigationStateProvider);
    final locationAsync = ref.watch(locationStateProvider);

    if (!navState.isActive || navState.target == null) {
      return const SizedBox.shrink();
    }

    final userPosition = locationAsync.value;
    if (userPosition == null) return const SizedBox.shrink();

    return PolylineLayer(
      polylines: [
        Polyline(
          points: [userPosition, navState.target!],
          strokeWidth: 4,
          color: Theme.of(context).colorScheme.primary.withAlpha(180),
          pattern: const StrokePattern.dotted(),
          strokeCap: StrokeCap.round,
        ),
      ],
    );
  }
}
