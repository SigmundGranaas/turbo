import 'package:flutter/material.dart';
import 'package:flutter_map/flutter_map.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import 'package:turbo/features/navigation/data/navigation_state_notifier.dart';

class NavigationTargetMarker extends ConsumerWidget {
  const NavigationTargetMarker({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final navState = ref.watch(navigationStateProvider);

    if (!navState.isActive || navState.target == null) {
      return const SizedBox.shrink();
    }

    return MarkerLayer(
      markers: [
        Marker(
          point: navState.target!,
          width: 40,
          height: 40,
          child: Icon(
            Icons.flag_circle,
            color: Theme.of(context).colorScheme.error,
            size: 36,
          ),
        ),
      ],
    );
  }
}
