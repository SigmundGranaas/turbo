import 'package:flutter/material.dart';
import 'package:flutter_map/flutter_map.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:latlong2/latlong.dart';

import 'package:turbo/app/tokens.dart';
import 'package:turbo/core/widgets/app_pill.dart';
import 'package:turbo/core/widgets/map/controls/bottom_controls.dart';
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
/// against the tileserver (with a streaming preview).
class RoutePlanningPage extends ConsumerStatefulWidget {
  final LatLng initialCenter;
  final double initialZoom;

  /// Stops to seed (e.g. the long-pressed point, or [you, destination] for
  /// "route to here"). Solved immediately if ≥2.
  final List<LatLng> initialWaypoints;

  const RoutePlanningPage({
    super.key,
    required this.initialCenter,
    required this.initialZoom,
    this.initialWaypoints = const [],
  });

  @override
  ConsumerState<RoutePlanningPage> createState() => _RoutePlanningPageState();
}

class _RoutePlanningPageState extends ConsumerState<RoutePlanningPage>
    with TickerProviderStateMixin {
  late final MapController _mapController = MapController();

  // Keyed onto the map subtree so globalToLocal during a drag resolves
  // against the map, not the Scaffold.
  final GlobalKey _mapKey = GlobalKey();

  /// Index of the stop being dragged, or null. While set, the map's own
  /// pan/zoom is frozen so the gesture only moves the dot.
  int? _draggingIdx;

  @override
  void initState() {
    super.initState();
    if (widget.initialWaypoints.isNotEmpty) {
      WidgetsBinding.instance.addPostFrameCallback((_) {
        if (!mounted) return;
        final notifier = ref.read(routePlanningProvider.notifier);
        for (final wp in widget.initialWaypoints) {
          notifier.addWaypoint(wp);
        }
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

    final isMobile = MediaQuery.of(context).size.width < 600;
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
            // Live best-path-so-far preview, drawn under the final route.
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
            // Hide the stale final line while a fresh preview animates.
            RoutePolylineLayer(
              plan: state.previewGeometry.length >= 2 ? null : state.plan,
            ),
            MarkerLayer(
              markers: [
                for (var i = 0; i < wps.length; i++)
                  Marker(
                    point: wps[i],
                    width: 40,
                    height: 40,
                    alignment: Alignment.center,
                    child: RouteWaypointDot(
                      index: i,
                      isStart: i == 0,
                      isEnd: i == wps.length - 1 && wps.length > 1,
                      isDragging: _draggingIdx == i,
                      onDragStart: () => setState(() => _draggingIdx = i),
                      onDragUpdate: (d) => _onWaypointDrag(i, d),
                      onDragEnd: () {
                        if (_draggingIdx != null) {
                          setState(() => _draggingIdx = null);
                        }
                      },
                      onRemove: () => notifier.removeAt(i),
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
              child: Center(child: _HintPill(waypointCount: wps.length)),
            ),
            BottomControls(controls: const RoutePlanningSheet()),
          ],
        ),
      ),
    );
  }
}

/// Floating hint that teaches the gesture model, using the app's AppPill
/// chrome so it matches the navigation / mode indicators.
class _HintPill extends StatelessWidget {
  final int waypointCount;
  const _HintPill({required this.waypointCount});

  @override
  Widget build(BuildContext context) {
    final text = switch (waypointCount) {
      0 => 'Tap the map to set your start',
      1 => 'Tap to add your destination',
      _ => 'Drag a stop to adjust · long-press to remove',
    };
    return AppPill(
      padding: const EdgeInsets.symmetric(
          horizontal: AppSpacing.l, vertical: AppSpacing.s),
      child: Text(text, style: Theme.of(context).textTheme.labelMedium),
    );
  }
}
