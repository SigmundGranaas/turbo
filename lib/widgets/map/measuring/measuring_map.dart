import 'package:flutter/material.dart';
import 'package:flutter_map/flutter_map.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:latlong2/latlong.dart';
import 'package:map_app/widgets/map/controller/map_utility.dart';
import 'package:map_app/widgets/map/controls/bottom_controls.dart';
import 'package:map_app/widgets/map/controls/go_back_button.dart';
import 'package:map_app/widgets/map/controls/map_controls.dart';
import 'package:map_app/widgets/map/measuring/measuring_state_notifier.dart';
import '../controls/default_map_controls.dart';
import '../layers/tiles/tile_registry/tile_registry.dart';
import '../map_base.dart';
import 'measuring_controls.dart';
import 'measuring_line.dart';
import 'measuring_markers.dart';

final measuringStateProvider = StateNotifierProvider.autoDispose
    .family<MeasuringStateNotifier, MeasuringState, LatLng>(
      (ref, startPoint) {
    return MeasuringStateNotifier(startPoint: startPoint);
  },
);

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
    final List<Widget> controls =
    isMobile ? defaultMobileMapControls(_mapController, this) : defaultMapControls(_mapController, this);

    return Scaffold(
      body: MapBase(
        initialCenter: widget.initialPosition,
        initialZoom: widget.zoom,
        mapController: _mapController,
        onMapReady: () {
          WidgetsBinding.instance.addPostFrameCallback((_) {
            if (mounted) {
              animatedMapMove(
                  widget.startPoint, widget.zoom + 1, _mapController, this);
            }
          });
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
          MeasurePolyline(points: measuringState.points),
          MeasureMarkers(points: measuringState.points)
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
              canUndo: measuringState.points.length > 1,
              canReset: measuringState.points.length > 1,
            ),
          ),
        ],
        onTap: (tapPosition, point) => _handleMeasuringPress(point),
      ),
    );
  }

  void _handleMeasuringPress(LatLng point) {
    ref.read(measuringStateProvider(widget.startPoint).notifier).addPoint(point);
    animatedMapMove(point, _mapController.camera.zoom, _mapController, this);
  }
}