import 'package:flutter/material.dart';
import 'package:flutter_map/flutter_map.dart';

import '../models/route_models.dart';

/// Renders a planned route as a polyline on a [FlutterMap]. Drop it into
/// the map's `children` (it returns an empty box when [plan] is null or
/// degenerate). A thin casing stroke under the main line keeps the route
/// legible over busy terrain tiles.
class RoutePolylineLayer extends StatelessWidget {
  final RoutePlan? plan;

  /// Main stroke colour; defaults to the theme primary.
  final Color? color;

  /// Width of the main stroke, logical pixels.
  final double strokeWidth;

  const RoutePolylineLayer({
    super.key,
    required this.plan,
    this.color,
    this.strokeWidth = 5,
  });

  @override
  Widget build(BuildContext context) {
    final p = plan;
    if (p == null || p.geometry.length < 2) return const SizedBox.shrink();

    final main = color ?? Theme.of(context).colorScheme.primary;
    final casing = Theme.of(context).colorScheme.surface.withValues(alpha: 0.9);

    return PolylineLayer(
      polylines: [
        Polyline(
          points: p.geometry,
          strokeWidth: strokeWidth + 3,
          color: casing,
          strokeCap: StrokeCap.round,
          strokeJoin: StrokeJoin.round,
        ),
        Polyline(
          points: p.geometry,
          strokeWidth: strokeWidth,
          color: main,
          strokeCap: StrokeCap.round,
          strokeJoin: StrokeJoin.round,
        ),
      ],
    );
  }
}
