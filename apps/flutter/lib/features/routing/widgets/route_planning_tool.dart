import 'package:flutter/material.dart';
import 'package:flutter_map/flutter_map.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:latlong2/latlong.dart';

import 'package:turbo/core/widgets/dismissible_tool_sheet.dart';
import 'package:turbo/core/widgets/map/buttons/map_control_button_base.dart';
import 'package:turbo/features/map_view/api.dart';

import '../data/route_planning_notifier.dart';
import 'route_layer.dart';
import 'route_planning_sheet.dart';
import 'route_waypoint_markers.dart';

/// Index of the waypoint currently being dragged (or null). Drives both the
/// dot's enlarged appearance and freezing the map's pan so the gesture only
/// moves the dot. autoDispose: lives only while the tool is mounted.
final routePlanDragIndexProvider =
    NotifierProvider.autoDispose<RoutePlanDragIndexNotifier, int?>(
        RoutePlanDragIndexNotifier.new);

class RoutePlanDragIndexNotifier extends Notifier<int?> {
  @override
  int? build() => null;

  void setIndex(int? index) => state = index;
}

/// Route planning as an in-place [MapToolDescriptor] mounted on the single
/// shared map — replaces the old full-screen `RoutePlanningPage` with its own
/// second `MapController`. Reuses the existing layers, waypoint dots and
/// planning sheet verbatim; only the host changed. See
/// `docs/architecture/2026-06-composition-overhaul-plan.md` (Phase 4).
final routePlanningTool = MapToolDescriptor(
  id: routePlanningToolId,
  buildLayers: (ctx) =>
      [RoutePlanToolLayers(mapController: ctx.mapController)],
  buildOverlay: (ctx) => const RoutePlanToolOverlay(),
  // Tapping the drawn route inserts a movable via-point in the nearest gap;
  // tapping elsewhere appends a stop. (Tapping a dot is handled by the dot
  // itself — it removes that stop.)
  onMapTap: (ctx, point) {
    final notifier = ctx.ref.read(routePlanningProvider.notifier);
    final st = ctx.ref.read(routePlanningProvider);
    final geometry = (st.plan != null && st.plan!.geometry.length >= 2)
        ? st.plan!.geometry
        : st.previewGeometry;
    if (st.waypoints.length >= 2 &&
        geometry.length >= 2 &&
        _tapNearRoute(point, geometry, ctx.mapController.camera)) {
      notifier.insertWaypoint(point);
    } else {
      notifier.addWaypoint(point);
    }
  },
  interaction: (ctx) {
    final dragging = ctx.ref.watch(routePlanDragIndexProvider) != null;
    return InteractionOptions(
      flags: dragging
          ? InteractiveFlag.none
          : InteractiveFlag.all & ~InteractiveFlag.rotate,
      pinchZoomThreshold: 0.2,
      pinchMoveThreshold: 40,
    );
  },
  // Leaving the tool throws away the in-flight route so the next planning
  // session starts clean.
  onDeactivate: (ctx) => ctx.ref.read(routePlanningProvider.notifier).clear(),
);

const String routePlanningToolId = 'route_planning';

/// Whether a tapped [point] lands on (within ~26 px of) the drawn route — used
/// to distinguish "insert a via-point on the route" from "append a stop". The
/// route geometry is dense, so vertex proximity in screen space approximates
/// line proximity well enough for a finger tap.
bool _tapNearRoute(LatLng point, List<LatLng> geometry, MapCamera camera) {
  const thresholdPx = 26.0;
  final tapPx = camera.latLngToScreenOffset(point);
  for (final g in geometry) {
    if ((camera.latLngToScreenOffset(g) - tapPx).distance <= thresholdPx) {
      return true;
    }
  }
  return false;
}

/// The map layers for the planning tool: live preview, solved route, and the
/// draggable waypoint dots.
class RoutePlanToolLayers extends ConsumerWidget {
  final MapController mapController;
  const RoutePlanToolLayers({super.key, required this.mapController});

  void _onDrag(WidgetRef ref, int index, Offset globalPosition) {
    // The map fills the screen (no app bar), so the global drag position maps
    // directly to a screen offset on the map camera.
    final point = mapController.camera.screenOffsetToLatLng(globalPosition);
    ref.read(routePlanningProvider.notifier).moveWaypoint(index, point);
  }

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final state = ref.watch(routePlanningProvider);
    final notifier = ref.read(routePlanningProvider.notifier);
    final draggingIdx = ref.watch(routePlanDragIndexProvider);
    final wps = state.waypoints;

    return Stack(
      children: [
        if (state.previewGeometry.length >= 2)
          PolylineLayer(
            polylines: [
              Polyline(
                points: state.previewGeometry,
                strokeWidth: 4.5,
                color: const Color(0xFF15233A).withValues(alpha: 0.85),
                strokeCap: StrokeCap.round,
                strokeJoin: StrokeJoin.round,
              ),
            ],
          ),
        RoutePolylineLayer(
          plan: state.previewGeometry.length >= 2 ? null : state.plan,
        ),
        MarkerLayer(
          markers: [
            for (var i = 0; i < wps.length; i++)
              Marker(
                // Stable per-stop key so removing/inserting a stop re-keys the
                // dots by identity, not list index — otherwise Flutter reuses
                // elements positionally and a removed dot can linger ("ghost").
                key: ValueKey('wp_${wps[i].latitude}_${wps[i].longitude}'),
                point: wps[i],
                width: 40,
                height: 40,
                alignment: Alignment.center,
                child: RouteWaypointDot(
                  index: i,
                  isStart: i == 0,
                  isEnd: i == wps.length - 1 && wps.length > 1,
                  isDragging: draggingIdx == i,
                  onDragStart: () => ref
                      .read(routePlanDragIndexProvider.notifier)
                      .setIndex(i),
                  onDragUpdate: (globalPosition) =>
                      _onDrag(ref, i, globalPosition),
                  onDragEnd: () => ref
                      .read(routePlanDragIndexProvider.notifier)
                      .setIndex(null),
                  onRemove: () => notifier.removeAt(i),
                ),
              ),
          ],
        ),
      ],
    );
  }
}

/// The planning tool's overlay: a close button, a gesture hint, and the
/// planning sheet anchored to the bottom.
class RoutePlanToolOverlay extends ConsumerWidget {
  const RoutePlanToolOverlay({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final scheme = Theme.of(context).colorScheme;

    return Stack(
      children: [
        Positioned(
          top: 16,
          left: 16,
          child: MapControlButtonBase(
            onPressed: () =>
                ref.read(activeMapToolProvider.notifier).deactivate(),
            child: Icon(Icons.close, color: scheme.primary),
          ),
        ),
        // The planning sheet, centred and width-capped (the AppCardSurface
        // maxWidth only applies once it isn't forced full-width).
        Positioned(
          left: 0,
          right: 0,
          bottom: 0,
          child: SafeArea(
            top: false,
            child: Align(
              alignment: Alignment.bottomCenter,
              child: DismissibleToolSheet(
                onDismiss: () =>
                    ref.read(activeMapToolProvider.notifier).deactivate(),
                child: const RoutePlanningSheet(),
              ),
            ),
          ),
        ),
      ],
    );
  }
}
