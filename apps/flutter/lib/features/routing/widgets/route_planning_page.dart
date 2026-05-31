import 'package:flutter/material.dart';
import 'package:flutter_map/flutter_map.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:latlong2/latlong.dart';

import 'package:turbo/core/widgets/map/controls/default_map_controls.dart';
import 'package:turbo/core/widgets/map/controls/go_back_button.dart';
import 'package:turbo/core/widgets/map/controls/map_controls.dart';
import 'package:turbo/features/map_view/api.dart';
import 'package:turbo/features/tile_providers/api.dart';

import '../data/route_planning_notifier.dart';
import 'route_layer.dart';
import 'route_planning_sheet.dart';
import 'route_waypoint_markers.dart';

/// Full-screen interactive route planner: tap the map to drop stops, drag
/// a stop to adjust it, long-press to remove. The route re-solves live
/// against the tileserver (with a streaming preview). Summary + controls
/// live in a center-bottom Material 3 sheet over the map.
class RoutePlanningPage extends ConsumerStatefulWidget {
  final LatLng initialCenter;
  final double initialZoom;

  /// Optional first stop to seed (e.g. the long-pressed point that opened
  /// the planner).
  final LatLng? initialWaypoint;

  const RoutePlanningPage({
    super.key,
    required this.initialCenter,
    required this.initialZoom,
    this.initialWaypoint,
  });

  @override
  ConsumerState<RoutePlanningPage> createState() => _RoutePlanningPageState();
}

class _RoutePlanningPageState extends ConsumerState<RoutePlanningPage>
    with TickerProviderStateMixin {
  late final MapController _mapController = MapController();

  // Keyed onto the map subtree so globalToLocal during a drag resolves
  // against the map, not the Scaffold (which the app bar would offset).
  final GlobalKey _mapKey = GlobalKey();

  /// Index of the stop being dragged, or null. While set, the map's own
  /// pan/zoom is frozen so the gesture only moves the dot.
  int? _draggingIdx;

  @override
  void initState() {
    super.initState();
    final seed = widget.initialWaypoint;
    if (seed != null) {
      WidgetsBinding.instance.addPostFrameCallback((_) {
        if (mounted) ref.read(routePlanningProvider.notifier).addWaypoint(seed);
      });
    }
  }

  void _onWaypointDrag(int index, DragUpdateDetails details) {
    final box = _mapKey.currentContext?.findRenderObject();
    if (box is! RenderBox) return;
    final local = box.globalToLocal(details.globalPosition);
    final point = _mapController.camera.screenOffsetToLatLng(local);
    ref.read(routePlanningProvider.notifier).moveWaypoint(index, point);
  }

  @override
  Widget build(BuildContext context) {
    final baseLayers = ref.watch(tileRegistryProvider.notifier).getActiveLayers();
    final registry = ref.watch(tileRegistryProvider);
    final state = ref.watch(routePlanningProvider);
    final notifier = ref.read(routePlanningProvider.notifier);
    final scheme = Theme.of(context).colorScheme;

    final width = MediaQuery.of(context).size.width;
    final isMobile = width < 600;
    final controls = isMobile
        ? defaultMobileMapControls(_mapController, this)
        : defaultMapControls(_mapController, this);

    final wps = state.waypoints;

    return Scaffold(
      body: KeyedSubtree(
        key: _mapKey,
        child: MapBase(
          initialCenter: widget.initialCenter,
          initialZoom: widget.initialZoom,
          mapController: _mapController,
          onTap: (_, point) => notifier.addWaypoint(point),
          // Freeze the camera while dragging a stop so the gesture only
          // moves the dot.
          interactionOptions: InteractionOptions(
            flags: _draggingIdx != null
                ? InteractiveFlag.none
                : InteractiveFlag.all & ~InteractiveFlag.rotate,
            pinchZoomThreshold: 0.2,
            pinchMoveThreshold: 40,
          ),
          mapLayers: [
            ...baseLayers,
            ...registry.activeGlobalIds.map(
              (id) => RichAttributionWidget(
                animationConfig: const ScaleRAWA(),
                attributions: [
                  TextSourceAttribution(
                      registry.availableProviders[id]!.attributions),
                ],
              ),
            ),
            // Live best-path-so-far preview, drawn under the final route so
            // the solid line wins once the result lands.
            if (state.previewGeometry.length >= 2)
              PolylineLayer(
                polylines: [
                  Polyline(
                    points: state.previewGeometry,
                    strokeWidth: 4,
                    color: scheme.primary.withValues(alpha: 0.55),
                    strokeCap: StrokeCap.round,
                    strokeJoin: StrokeJoin.round,
                  ),
                ],
              ),
            // Hide the (stale) final line while a fresh preview animates,
            // so the handoff is preview → solid, never doubled.
            RoutePolylineLayer(
              plan: state.previewGeometry.length >= 2 ? null : state.plan,
            ),
            MarkerLayer(
              markers: [
                for (var i = 0; i < wps.length; i++)
                  Marker(
                    point: wps[i],
                    width: 38,
                    height: 38,
                    alignment: Alignment.center,
                    child: RouteWaypointDot(
                      index: i,
                      isStart: i == 0,
                      isEnd: i == wps.length - 1 && wps.length > 1,
                      isDragging: _draggingIdx == i,
                      onPanStart: () => setState(() => _draggingIdx = i),
                      onPanUpdate: (d) => _onWaypointDrag(i, d),
                      onPanEnd: () => setState(() => _draggingIdx = null),
                      onLongPress: () => notifier.removeAt(i),
                    ),
                  ),
              ],
            ),
          ],
          overlayWidgets: [
            MapControls(controls: controls, top: 16),
            const Positioned(top: 16, left: 16, child: GoBackButton()),
            Positioned(
              top: 16,
              left: 0,
              right: 0,
              child: Center(child: _HintChip(waypointCount: wps.length)),
            ),
            Positioned(
              left: 0,
              right: 0,
              bottom: 0,
              child: SafeArea(
                top: false,
                child: Align(
                  alignment: Alignment.bottomCenter,
                  child: ConstrainedBox(
                    constraints: const BoxConstraints(maxWidth: 560),
                    child: const RoutePlanningSheet(),
                  ),
                ),
              ),
            ),
          ],
        ),
      ),
    );
  }
}

/// Small floating hint that teaches the gesture model without stealing
/// space from the bottom sheet.
class _HintChip extends StatelessWidget {
  final int waypointCount;
  const _HintChip({required this.waypointCount});

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final text = switch (waypointCount) {
      0 => 'Tap the map to set your start',
      1 => 'Tap to add your destination',
      _ => 'Drag a stop to adjust · long-press to remove',
    };
    return Container(
      margin: const EdgeInsets.symmetric(horizontal: 64),
      padding: const EdgeInsets.symmetric(horizontal: 14, vertical: 8),
      decoration: BoxDecoration(
        color: theme.colorScheme.surface.withValues(alpha: 0.92),
        borderRadius: BorderRadius.circular(20),
        boxShadow: [
          BoxShadow(
            color: Colors.black.withValues(alpha: 0.1),
            blurRadius: 4,
            offset: const Offset(0, 1),
          ),
        ],
      ),
      child: Text(
        text,
        textAlign: TextAlign.center,
        style: theme.textTheme.labelMedium,
      ),
    );
  }
}
