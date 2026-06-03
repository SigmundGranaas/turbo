import 'package:flutter/material.dart';
import 'package:flutter_map/flutter_map.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import 'package:turbo/core/geo/geo_metrics.dart';
import 'package:turbo/core/location/location_state.dart';
import 'package:turbo/core/widgets/map/map_line_style.dart';

import '../data/active_journey_notifier.dart';
import '../models/active_journey.dart';

/// Renders the path being followed during an active journey, live against the
/// user's position: the route line, a dotted "you are here → back on track"
/// connector when the user has strayed, and a destination pin. The current
/// location dot itself is drawn by the shared CurrentLocationLayer, so this
/// layer is purely the path overlay.
class JourneyPathLayer extends ConsumerWidget {
  const JourneyPathLayer({super.key});

  /// Beyond this perpendicular distance (m) we draw the off-route connector.
  static const double _offRouteThresholdM = 25;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final journey = ref.watch(activeJourneyProvider);
    if (journey.kind != JourneyKind.followingPath || journey.path == null) {
      return const SizedBox.shrink();
    }
    final path = journey.path!;
    if (path.points.length < 2) return const SizedBox.shrink();

    final pos = ref.watch(locationStateProvider).value;
    final progress = pos == null ? null : GeoMetrics.progress(path.points, pos);

    return Stack(
      children: [
        PolylineLayer(
          polylines: [
            // White casing under the line, matching the route line, so the
            // followed path reads identically and stays legible over terrain.
            Polyline(
              points: path.points,
              strokeWidth: 8,
              color: MapLineStyle.casing,
              strokeCap: StrokeCap.round,
              strokeJoin: StrokeJoin.round,
            ),
            Polyline(
              points: path.points,
              strokeWidth: 5,
              color: MapLineStyle.path,
              strokeCap: StrokeCap.round,
              strokeJoin: StrokeJoin.round,
            ),
          ],
        ),
        if (pos != null &&
            progress != null &&
            progress.offRouteM > _offRouteThresholdM)
          PolylineLayer(
            polylines: [
              Polyline(
                points: [pos, progress.snapped],
                strokeWidth: 3,
                color: MapLineStyle.warning.withValues(alpha: 0.85),
                pattern: const StrokePattern.dotted(),
                strokeCap: StrokeCap.round,
              ),
            ],
          ),
        MarkerLayer(
          markers: [
            Marker(
              point: path.points.last,
              width: 32,
              height: 32,
              alignment: Alignment.topCenter,
              child: Icon(Icons.flag, color: MapLineStyle.path, size: 30),
            ),
          ],
        ),
      ],
    );
  }
}
