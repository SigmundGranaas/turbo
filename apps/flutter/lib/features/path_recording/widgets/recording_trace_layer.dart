import 'package:flutter/material.dart';
import 'package:flutter_map/flutter_map.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import 'package:turbo/core/widgets/map/map_line_style.dart';
import '../data/recording_notifier.dart';

/// Live polyline of the in-flight recording. Renders the points the recorder
/// has accumulated so far so the user can see their track grow as they walk.
///
/// Mount in the map's layer stack — auto-collapses to an empty `SizedBox`
/// when no recording is active (or fewer than 2 fixes exist), so it costs
/// nothing in the idle case.
class RecordingTraceLayer extends ConsumerWidget {
  const RecordingTraceLayer({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final rec = ref.watch(recordingNotifierProvider);
    if (!rec.isActive || rec.points.length < 2) {
      return const SizedBox.shrink();
    }
    final color = MapLineStyle.recording;
    return PolylineLayer(
      polylines: [
        Polyline(
          points: rec.points,
          color: color,
          strokeWidth: 5,
          // White underlay improves legibility against varied tile backgrounds.
          borderColor: Colors.white.withValues(alpha: 0.85),
          borderStrokeWidth: 2,
        ),
      ],
    );
  }
}
