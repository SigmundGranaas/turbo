import 'package:flutter/material.dart' hide CatmullRomSpline;
import 'package:flutter_map/flutter_map.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:latlong2/latlong.dart';
import 'package:turbo/features/measuring/data/measuring_state.dart';
import 'package:turbo/features/measuring/data/measuring_state_notifier.dart';
import 'package:turbo/features/measuring/models/measure_point_type.dart';
import 'package:turbo/features/measuring/util/catmull_rom_spline.dart';
import 'package:turbo/features/measuring/widgets/measuring_controls.dart';
import 'package:turbo/features/measuring/widgets/measuring_line.dart';
import 'package:turbo/features/measuring/widgets/measuring_markers.dart';
import 'package:turbo/widgets/map/controller/map_utility.dart';
import 'package:turbo/widgets/map/controls/bottom_controls.dart';
import 'package:turbo/widgets/map/controls/default_map_controls.dart';
import 'package:turbo/widgets/map/controls/go_back_button.dart';
import 'package:turbo/widgets/map/controls/map_controls.dart';
import 'package:turbo/widgets/map/layers/tiles/tile_registry/tile_registry.dart';
import 'package:turbo/widgets/map/map_base.dart';

final measuringStateProvider = StateNotifierProvider.autoDispose
    .family<MeasuringStateNotifier, MeasuringState, LatLng>(
      (ref, startPoint) {
    return MeasuringStateNotifier(startPoint: startPoint);
  },
);

class MeasuringMapPage extends ConsumerStatefulWidget {
  final LatLng initialPosition;
  final double zoom;
  final LatLng startPoint;

  const MeasuringMapPage({
    super.key,
    required this.initialPosition,
    required this.startPoint,
    required this.zoom,
  });

  @override
  ConsumerState<MeasuringMapPage> createState() => _MeasuringMapPageState();
}

class _MeasuringMapPageState extends ConsumerState<MeasuringMapPage>
    with TickerProviderStateMixin {
  late MapController _mapController;
  bool _isPointerDown = false;
  Offset? _lastPointerScreenPos;

  @override
  void initState() {
    super.initState();
    _mapController = MapController();
  }

  @override
  Widget build(BuildContext context) {
    final layers = ref.watch(tileRegistryProvider.notifier).getActiveLayers();
    final registry = ref.watch(tileRegistryProvider);

    final measuringNotifier =
    ref.watch(measuringStateProvider(widget.startPoint).notifier);
    final measuringState = ref.watch(measuringStateProvider(widget.startPoint));

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
        // The onMapReady callback is removed to prevent test instability.
        // The map already initializes at the correct location.
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
          MapControls(controls: controls),
          const Positioned(top: 16, left: 16, child: GoBackButton()),
          BottomControls(
            controls: MeasuringControls(
              distance: measuringState.totalDistance,
              onReset: measuringNotifier.reset,
              onUndo: measuringNotifier.undoLastPoint,
              onFinish: () => Navigator.of(context).pop(),
              onToggleSmoothing: measuringNotifier.toggleSmoothing,
              onToggleDrawing: measuringNotifier.toggleDrawing,
              onToggleIntermediatePoints:
              measuringNotifier.toggleIntermediatePoints,
              onSensitivityChanged: measuringNotifier.setDrawSensitivity,
              canUndo: measuringState.points.length > 1,
              canReset: measuringState.points.length > 1,
              isSmoothing: measuringState.isSmoothing,
              isDrawing: measuringState.isDrawing,
              showIntermediatePoints: measuringState.showIntermediatePoints,
              drawSensitivity: measuringState.drawSensitivity,
            ),
          ),
        ],
        onTap: measuringState.isDrawing
            ? null
            : (tapPosition, point) => _handleMapTap(point),
        onPointerDown: measuringState.isDrawing ? _onPointerDown : null,
        onPointerMove: measuringState.isDrawing ? _onPointerMove : null,
        onPointerUp: measuringState.isDrawing ? _onPointerUp : null,
        interactionOptions: InteractionOptions(
          flags: measuringState.isDrawing
              ? InteractiveFlag.none
              : InteractiveFlag.all,
        ),
      ),
    );
  }

  void _handleMapTap(LatLng point) {
    ref
        .read(measuringStateProvider(widget.startPoint).notifier)
        .addPoint(point);
    animatedMapMove(point, _mapController.camera.zoom, _mapController, this);
  }

  void _onPointerDown(PointerDownEvent event, LatLng point) {
    setState(() {
      _isPointerDown = true;
      _lastPointerScreenPos = event.localPosition;
    });
    ref.read(measuringStateProvider(widget.startPoint).notifier).addPoint(point);
  }

  void _onPointerMove(PointerMoveEvent event, LatLng point) {
    if (!_isPointerDown || _lastPointerScreenPos == null) return;
    final sensitivity =
        ref.read(measuringStateProvider(widget.startPoint)).drawSensitivity;
    final distance = (event.localPosition - _lastPointerScreenPos!).distance;
    if (distance > sensitivity) {
      ref
          .read(measuringStateProvider(widget.startPoint).notifier)
          .addPoint(point);
      setState(() {
        _lastPointerScreenPos = event.localPosition;
      });
    }
  }

  void _onPointerUp(PointerUpEvent event, LatLng point) {
    setState(() {
      _isPointerDown = false;
      _lastPointerScreenPos = null;
    });
  }
}