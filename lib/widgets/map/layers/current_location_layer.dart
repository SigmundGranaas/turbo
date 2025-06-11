import 'package:flutter/material.dart';
import 'package:flutter_map/flutter_map.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../../data/state/providers/location_state.dart';

class CurrentLocationLayer extends ConsumerWidget {
  const CurrentLocationLayer({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final locationState = ref.watch(locationStateProvider);

    // This .when() block correctly handles all possible states of the AsyncNotifier.
    return locationState.when(
      data: (location) {
        // If data is available but null (e.g., on Linux), show nothing.
        if (location == null) return const SizedBox.shrink();

        // If we have a location, show the marker.
        return MarkerLayer(
          markers: [
            Marker(
              width: 60.0,
              height: 60.0,
              point: location,
              child: const CurrentLocationMarker(),
            ),
          ],
        );
      },
      // While loading or if there's an error, show nothing.
      loading: () => const SizedBox.shrink(),
      error: (error, stack) => const SizedBox.shrink(),
    );
  }
}

class CurrentLocationMarker extends StatelessWidget {
  const CurrentLocationMarker({super.key});

  @override
  Widget build(BuildContext context) {
    return Stack(
      alignment: Alignment.center,
      children: [
        // Outer circle (blue shadow)
        Container(
          width: 32,
          height: 32,
          decoration: BoxDecoration(
            shape: BoxShape.circle,
            color: Colors.lightBlue.withOpacity(0.3),
          ),
        ),
        // Blue dot
        Container(
          width: 16,
          height: 16,
          decoration: BoxDecoration(
            shape: BoxShape.circle,
            color: Colors.lightBlue,
            border: Border.all(color: Colors.white, width: 2),
          ),
        ),
      ],
    );
  }
}