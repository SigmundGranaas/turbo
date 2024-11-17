import 'package:flutter/material.dart';
import 'package:flutter_map/flutter_map.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:latlong2/latlong.dart';
import 'package:map_app/widgets/map/controller/map_utility.dart';
import 'package:map_app/widgets/map/controls/bottom_controls.dart';
import 'package:map_app/widgets/map/controls/go_back_button.dart';
import 'package:map_app/widgets/map/controls/map_controls.dart';
import 'package:map_app/widgets/map/measuring/measure_point_collection.dart';
import '../controls/default_map_controls.dart';
import '../layers/tiles/tile_registry/tile_registry.dart';
import '../map_base.dart';
import 'measure_point.dart';
import 'measuring_controls.dart';
import 'measuring_line.dart';
import 'measuring_markers.dart';

class MeasuringControllerPage extends ConsumerStatefulWidget {
  final LatLng initialPosition;
  final double zoom;
  final LatLng startPoint;

  const MeasuringControllerPage({
    super.key,
    required this.initialPosition,
    required this.startPoint,
    required this.zoom,
  });

  @override
  ConsumerState<MeasuringControllerPage> createState() =>
      MeasuringControllerPageState();
}

class MeasuringControllerPageState extends ConsumerState<MeasuringControllerPage>
    with TickerProviderStateMixin {
  late MapController _mapController;
  List<MeasurePoint> _measurePoints = [];
  double _totalDistance = 0;
  MeasurePointCollection collection = MeasurePointCollection();

  @override
  void initState() {
    super.initState();
    _mapController = MapController();
    _addPoint(widget.startPoint, animate: false);
  }

  @override
  Widget build(BuildContext context) {
    final layers = ref.watch(tileRegistryProvider.notifier).getActiveLayers();
    final registry = ref.watch(tileRegistryProvider);

    final controls = defaultMapControls(_mapController, this);

    return Scaffold(
      body: MapBase(
        initialCenter: widget.initialPosition,
        initialZoom: widget.zoom,
        mapController: _mapController,
        onMapReady: () => {
          WidgetsBinding.instance.addPostFrameCallback((_) {
            animatedMapMove(widget.startPoint, widget.zoom + 1, _mapController, this);
          })
        },
        mapLayers: [
          ...layers,
          ...registry.activeGlobalIds.map((id) => RichAttributionWidget(
            animationConfig: const ScaleRAWA(),
            attributions: [
              TextSourceAttribution(
                  registry.availableProviders[id]!.attributions),
            ],
          )),
          MeasurePolyline(points: _measurePoints),
          MeasureMarkers(points: _measurePoints)
        ],
        overlayWidgets: [
          MapControls(controls: controls),
          const Positioned(top: 16, left: 16, child: GoBackButton()),
          BottomControls(
            controls:
              MeasuringControls(
                distance: _totalDistance,
                onReset: _resetMeasurement,
                onUndo: _undoLastPoint,
                onFinish: () => Navigator.of(context).pop(),
              ),
          ),
        ],
        onTap: _handleMeasuringPress,
      ),
    );
  }

  void _handleMeasuringPress(TapPosition tapPosition, LatLng point) {
    _addPoint(point);
  }

  void _addPoint(LatLng point, {bool animate = true}) {
    collection.addPoint(point);
    sync();

    if (animate) {
      animatedMapMove(point, _mapController.camera.zoom, _mapController, this);
    }
  }

  void _resetMeasurement() {
    collection.reset(widget.startPoint);
    sync();
  }

  void _undoLastPoint() {
    if (collection.points.length <= 1) {
      return;
    }

    collection.undoLastPoint();
    sync();
  }

  /// Logic and state has been split to make it easier to test.
  /// Because of this, we need to perform manual sync when updating data to make sure it is reflected in the UI.
  void sync(){
    setState(() {
      _measurePoints = collection.points;
      _totalDistance = collection.totalDistance;
    });
  }
}