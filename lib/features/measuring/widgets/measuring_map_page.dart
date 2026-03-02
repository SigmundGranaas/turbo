import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart' hide CatmullRomSpline;
import 'package:flutter_map/flutter_map.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:latlong2/latlong.dart';
import 'package:turbo/features/measuring/data/measuring_state.dart';
import 'package:turbo/features/measuring/data/measuring_state_notifier.dart';
import 'package:turbo/features/measuring/models/measure_point_type.dart';
import 'package:turbo/core/util/catmull_rom_spline.dart';
import 'package:turbo/features/measuring/widgets/measuring_controls.dart';
import 'package:turbo/features/measuring/widgets/measuring_line.dart';
import 'package:turbo/features/measuring/widgets/measuring_markers.dart';
import 'package:turbo/features/saved_paths/widgets/save_path_sheet.dart';
import 'package:turbo/features/settings/api.dart';
import 'package:turbo/core/widgets/map/controller/map_utility.dart';
import 'package:turbo/core/widgets/map/controls/bottom_controls.dart';
import 'package:turbo/core/widgets/map/controls/default_map_controls.dart';
import 'package:turbo/core/widgets/map/controls/go_back_button.dart';
import 'package:turbo/core/widgets/map/controls/map_controls.dart';
import 'package:turbo/l10n/app_localizations.dart';

import '../../map_view/widgets/map_base.dart';
import '../../tile_providers/api.dart';
import '../data/freehand_gesture_handler.dart';
import '../util/edge_pan_handler.dart';

final measuringStateProvider = NotifierProvider.autoDispose<MeasuringStateNotifier, MeasuringState>(
  MeasuringStateNotifier.new,
);

class MeasuringMapPage extends ConsumerStatefulWidget {
  final LatLng initialPosition;
  final double zoom;

  const MeasuringMapPage({
    super.key,
    required this.initialPosition,
    required this.zoom,
  });

  @override
  ConsumerState<MeasuringMapPage> createState() => _MeasuringMapPageState();
}

class _MeasuringMapPageState extends ConsumerState<MeasuringMapPage>
    with TickerProviderStateMixin {
  late final MapController _mapController;
  late final EdgePanHandler _edgePanHandler;
  late final FreehandGestureHandler _gestureHandler;

  bool get _isMobilePlatform =>
      defaultTargetPlatform == TargetPlatform.android ||
      defaultTargetPlatform == TargetPlatform.iOS;

  @override
  void initState() {
    super.initState();
    _mapController = MapController();

    _edgePanHandler = EdgePanHandler(mapController: _mapController);

    _gestureHandler = FreehandGestureHandler(
      notifier: ref.read(measuringStateProvider.notifier),
      getSensitivity: () => ref.read(settingsProvider).value?.drawSensitivity ?? 15.0,
      onEdgePanStop: () => _edgePanHandler.stop(),
      onMove: _isMobilePlatform
          ? (_) {}
          : (pos) => _edgePanHandler.handlePointerMove(pos, MediaQuery.of(context).size),
    );
  }

  @override
  void dispose() {
    _edgePanHandler.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final layers = ref.watch(tileRegistryProvider.notifier).getActiveLayers();
    final registry = ref.watch(tileRegistryProvider);
    final measuringState = ref.watch(measuringStateProvider);
    final measuringNotifier = ref.watch(measuringStateProvider.notifier);

    final width = MediaQuery.of(context).size.width;
    final isMobile = width < 600;
    final List<Widget> controls = isMobile
        ? defaultMobileMapControls(_mapController, this)
        : defaultMapControls(_mapController, this);

    final allPoints = measuringState.points;
    final pointsToRenderForMarkers = measuringState.showIntermediatePoints
        ? allPoints
        : allPoints
        .where((p) =>
    p.type == MeasurePointType.start ||
        p.type == MeasurePointType.end)
        .toList();

    final rawPointsForLine = allPoints.map((p) => p.point).toList();
    final polylinePoints = measuringState.isSmoothing
        ? CatmullRomSpline(controlPoints: rawPointsForLine).generate()
        : rawPointsForLine;

    return Scaffold(
      body: MapBase(
        initialCenter: widget.initialPosition,
        initialZoom: widget.zoom,
        mapController: _mapController,
        mapLayers: [
          ...layers,
          ...registry.activeGlobalIds.map((id) => RichAttributionWidget(
            animationConfig: const ScaleRAWA(),
            attributions: [
              TextSourceAttribution(
                  registry.availableProviders[id]!.attributions),
            ],
          )),
          MeasurePolyline(points: polylinePoints),
          MeasureMarkers(points: pointsToRenderForMarkers)
        ],
        overlayWidgets: [
          MapControls(controls: controls, top: 16),
          const Positioned(
            top: 16,
            left: 16,
            child: GoBackButton(),
          ),
          BottomControls(
            controls: MeasuringControls(
              distance: measuringState.totalDistance,
              onReset: () {
                _gestureHandler.reset();
                measuringNotifier.reset();
              },
              onUndo: measuringNotifier.undoLastPoint,
              onFinish: () => _handleFinish(measuringState),
              onToggleSmoothing: measuringNotifier.toggleSmoothing,
              onToggleDrawing: measuringNotifier.toggleDrawing,
              onToggleIntermediatePoints:
              measuringNotifier.toggleIntermediatePoints,
              canUndo: measuringState.points.isNotEmpty,
              canReset: measuringState.points.isNotEmpty,
              canSave: measuringState.points.length >= 2,
              isSmoothing: measuringState.isSmoothing,
              isDrawing: measuringState.isDrawing,
              showIntermediatePoints: measuringState.showIntermediatePoints,
            ),
          ),
        ],
        onTap: measuringState.isDrawing
            ? null
            : (tapPosition, point) => _handleMapTap(point),
        onPointerDown: measuringState.isDrawing ? _gestureHandler.handlePointerDown : null,
        onPointerMove: measuringState.isDrawing ? _gestureHandler.handlePointerMove : null,
        onPointerUp: measuringState.isDrawing ? _gestureHandler.handlePointerUp : null,
        interactionOptions: InteractionOptions(
          flags: measuringState.isDrawing
              ? (InteractiveFlag.all & ~InteractiveFlag.drag & ~InteractiveFlag.rotate)
              : (InteractiveFlag.all & ~InteractiveFlag.rotate),
          pinchZoomThreshold: 0.2,
          pinchMoveThreshold: 40,
        ),
      ),
    );
  }

  void _handleFinish(MeasuringState measuringState) async {
    if (measuringState.points.length < 2) {
      final l10n = context.l10n;
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(
          content: Text(l10n.needMorePoints),
          behavior: SnackBarBehavior.floating,
          shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(12)),
          margin: const EdgeInsets.all(16),
        ),
      );
      return;
    }

    final points = measuringState.points.map((p) => p.point).toList();
    final distance = measuringState.totalDistance;

    final saved = await showModalBottomSheet<bool>(
      context: context,
      isScrollControlled: true,
      useSafeArea: true,
      builder: (context) => SavePathSheet(
        points: points,
        distance: distance,
        isSmoothing: measuringState.isSmoothing,
      ),
    );

    if (!mounted) return;
    if (saved == true) {
      Navigator.of(context).pop(true);
    } else if (saved == false) {
      Navigator.of(context).pop(false);
    }
    // saved == null (swipe-down dismiss) → do nothing, user continues editing
  }

  void _handleMapTap(LatLng point) {
    ref
        .read(measuringStateProvider.notifier)
        .addPoint(point);
    animatedMapMove(point, _mapController.camera.zoom, _mapController, this);
  }
}
