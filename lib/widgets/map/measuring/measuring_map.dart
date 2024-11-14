import 'package:flutter/material.dart';
import 'package:flutter_map/flutter_map.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:latlong2/latlong.dart';
import 'package:map_app/widgets/map/controller/map_utility.dart';
import 'package:map_app/widgets/map/controls/bottom_controls.dart';
import 'package:map_app/widgets/map/controls/map_controls.dart';

import '../buttons/compass.dart';
import '../buttons/location_button.dart';
import '../buttons/map_layer_button.dart';
import '../buttons/plus_minus_buttons.dart';
import '../controls/default_map_controls.dart';
import '../layers/tiles/tile_registry/tile_registry.dart';
import '../map_base.dart';
import 'measure_point.dart';
import 'measure_point_type.dart';
import 'measuring_controls.dart';

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
  bool _isMapReady = true;

  @override
  void initState() {
    super.initState();
    _mapController = MapController();
    _addPoint(widget.startPoint, animate: false);

    // Fix initial map loading and position
    WidgetsBinding.instance.addPostFrameCallback((_) {
      _mapController.move(
        widget.initialPosition,
        widget.zoom + 0.1,
      );
      setState(() => _isMapReady = true);
    });
  }

  @override
  Widget build(BuildContext context) {
    final layers = ref.watch(tileRegistryProvider.notifier).getActiveLayers();
    final registry = ref.watch(tileRegistryProvider);

    final controls = defaultMapControls(_mapController, this);

    if (!_isMapReady) {
      return const Scaffold(
        body: Center(child: CircularProgressIndicator()),
      );
    }

    return Scaffold(
      body: MapBase(
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
          PolylineLayer(
            polylines: [
              Polyline(
                points: _measurePoints.map((p) => p.point).toList(),
                strokeWidth: 3,
                color: Theme.of(context).primaryColor.withOpacity(0.8),
                strokeCap: StrokeCap.round,
                strokeJoin: StrokeJoin.round,
              ),
            ],
          ),
          MarkerLayer(
            markers: _buildMarkers(),
          ),
        ],
        overlayWidgets: [
          MapControls(controls: controls),
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

  List<Marker> _buildMarkers() {
    return _measurePoints.map((point) {
      return Marker(
        point: point.point,
        child: TweenAnimationBuilder<double>(
          duration: const Duration(milliseconds: 300),
          tween: Tween(begin: 0.0, end: 1.0),
          curve: Curves.elasticOut,
          builder: (context, value, child) {
            return Transform.scale(
              scale: value,
              child: Icon(
                point.type.icon,
                color: Theme.of(context).primaryColor,
                size: 16,
              ),
            );
          },
        ),
      );
    }).toList();
  }

  void _handleMeasuringPress(TapPosition tapPosition, LatLng point) {
    _addPoint(point);
  }

  void _addPoint(LatLng point, {bool animate = true}) {
    setState(() {
      final newType = _measurePoints.isEmpty
          ? MeasurePointType.start
          : _measurePoints.length == 1
          ? MeasurePointType.end
          : MeasurePointType.middle;

      // Update previous end point to middle if it exists
      if (newType == MeasurePointType.middle) {
        final lastIndex = _measurePoints.length - 1;
        _measurePoints[lastIndex] = MeasurePoint(
          point: _measurePoints[lastIndex].point,
          type: MeasurePointType.middle,
        );
      }

      _measurePoints.add(MeasurePoint(point: point, type: newType));

      if (_measurePoints.length > 1) {
        _totalDistance += const Distance().distance(
          _measurePoints[_measurePoints.length - 2].point,
          point,
        );
      }
    });

    if (animate) {
      animatedMapMove(point, _mapController.camera.zoom, _mapController, this);
    }
  }

  void _resetMeasurement() {
    setState(() {
      _measurePoints = [
        MeasurePoint(point: widget.startPoint, type: MeasurePointType.start)
      ];
      _totalDistance = 0;
    });
  }

  void _undoLastPoint() {
    if (_measurePoints.length <= 1) {
      return;
    }

    setState(() {
      final removedPoint = _measurePoints.removeLast();
      if (_measurePoints.length > 1) {
        // Update the new last point to be an end point
        final lastIndex = _measurePoints.length - 1;
        _measurePoints[lastIndex] = MeasurePoint(
          point: _measurePoints[lastIndex].point,
          type: MeasurePointType.end,
        );
      }

      _totalDistance -= const Distance().distance(
        _measurePoints.last.point,
        removedPoint.point,
      );
    });
  }
}