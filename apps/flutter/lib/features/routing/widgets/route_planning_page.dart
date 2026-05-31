import 'package:flutter/material.dart';
import 'package:flutter_map/flutter_map.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:latlong2/latlong.dart';

import 'package:turbo/core/widgets/map/controller/map_utility.dart';
import 'package:turbo/core/widgets/map/controls/default_map_controls.dart';
import 'package:turbo/core/widgets/map/controls/go_back_button.dart';
import 'package:turbo/core/widgets/map/controls/map_controls.dart';
import 'package:turbo/features/map_view/api.dart';
import 'package:turbo/features/tile_providers/api.dart';

import '../data/route_planning_notifier.dart';
import 'route_layer.dart';
import 'route_planning_sheet.dart';
import 'route_waypoint_markers.dart';

/// Full-screen interactive route planner: tap the map to drop stops, pick
/// a preset, and the route re-solves live against the tileserver. The
/// summary + controls live in a center-bottom Material 3 sheet over the
/// map (debug/diagnostics intentionally omitted — that lives in the admin
/// SPA, not the app).
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

  @override
  Widget build(BuildContext context) {
    final baseLayers = ref.watch(tileRegistryProvider.notifier).getActiveLayers();
    final registry = ref.watch(tileRegistryProvider);
    final state = ref.watch(routePlanningProvider);
    final notifier = ref.read(routePlanningProvider.notifier);

    final width = MediaQuery.of(context).size.width;
    final isMobile = width < 600;
    final controls = isMobile
        ? defaultMobileMapControls(_mapController, this)
        : defaultMapControls(_mapController, this);

    return Scaffold(
      body: MapBase(
        initialCenter: widget.initialCenter,
        initialZoom: widget.initialZoom,
        mapController: _mapController,
        onTap: (_, point) => _addStop(point, notifier),
        interactionOptions: const InteractionOptions(
          flags: InteractiveFlag.all & ~InteractiveFlag.rotate,
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
          // Live best-path-so-far preview (dotted), drawn under the final
          // route so the solid line wins once the result lands.
          if (state.previewGeometry.length >= 2)
            PolylineLayer(
              polylines: [
                Polyline(
                  points: state.previewGeometry,
                  strokeWidth: 3,
                  color: Theme.of(context)
                      .colorScheme
                      .primary
                      .withValues(alpha: 0.45),
                  pattern: const StrokePattern.dotted(),
                  strokeCap: StrokeCap.round,
                ),
              ],
            ),
          RoutePolylineLayer(plan: state.plan),
          RouteWaypointMarkers(
            waypoints: state.waypoints,
            onTapIndex: notifier.removeAt,
          ),
        ],
        overlayWidgets: [
          MapControls(controls: controls, top: 16),
          const Positioned(top: 16, left: 16, child: GoBackButton()),
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
    );
  }

  void _addStop(LatLng point, RoutePlanningNotifier notifier) {
    notifier.addWaypoint(point);
    animatedMapMove(point, _mapController.camera.zoom, _mapController, this);
  }
}
