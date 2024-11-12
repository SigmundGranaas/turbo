import 'package:flutter/material.dart';
import 'package:flutter_map/flutter_map.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:latlong2/latlong.dart';
import 'package:map_app/widgets/map/plus_minus_buttons.dart';

import '../../data/model/marker.dart' as marker_model;
import '../marker/create_location_sheet.dart';
import 'compass.dart';
import 'layers/current_location_layer.dart';
import 'layers/map_layer_button.dart';
import 'layers/saved_markers_layer.dart';
import 'layers/tiles/registry/tile_registry.dart';
import 'location_button.dart';

class MapControllerPage extends ConsumerStatefulWidget {

  const MapControllerPage({super.key});

  @override
  ConsumerState<MapControllerPage> createState() => MapControllerPageState();
}

class MapControllerPageState extends ConsumerState<MapControllerPage>
    with TickerProviderStateMixin {
  late final MapController _mapController = MapController();

  @override
  void initState() {
    super.initState();
  }

  @override
  Widget build(BuildContext context) {
    final layers = ref.watch(tileRegistryProvider.notifier).getActiveLayers();
    final registry = ref.watch(tileRegistryProvider);

    return Scaffold(
      body: Stack(
        children: [
          FlutterMap(
            mapController: _mapController,
            options: MapOptions(
              initialCenter: const LatLng(65.0, 13.0),
              initialZoom: 5,
              maxZoom: 20,
              minZoom: 3,
              onTap: (tapPosition, point) => _handleMapTap(context, point),
              interactionOptions: const InteractionOptions(
                flags: InteractiveFlag.all,
                enableMultiFingerGestureRace: true,
                pinchZoomThreshold: 0.2,
                pinchMoveThreshold: 40,
                rotationThreshold: 5.0,
              ),
            ),
            children: [
              ...layers,

              if (registry.selectedGlobalId != null)
                RichAttributionWidget(
                  animationConfig: const ScaleRAWA(),
                  attributions: [
                    TextSourceAttribution(
                        registry.availableProviders[registry.selectedGlobalId]!.attributions
                    ),
                  ],
                ),
              ...registry.activeLocalIds.map((id) =>
                  RichAttributionWidget(
                    animationConfig: const ScaleRAWA(),
                    attributions: [
                      TextSourceAttribution(
                          registry.availableProviders[id]!.attributions
                      ),
                    ],
                  ),
              ),

              const CurrentLocationLayer(),
              LocationMarkers(
                onMarkerTap: (location) => _showEditSheet(context, location),
              ),
            ],
          ),
          Positioned(
            top: 80,
            right: 16,
            child: Column(
              children: [
                const MapLayerButton(),
                LocationButton(mapController: _mapController),
                CustomMapCompass(mapController: _mapController),
                PlusMinusButtons(
                  onZoomIn: _onZoomIn,
                  onZoomOut: _onZoomOut,
                ),
              ],
            ),
          ),
        ],
      ),
    );
  }

  void _showEditSheet(BuildContext context, marker_model.Marker? marker,
      {LatLng? newLocation}) async {
    if (newLocation != null) {
      _animatedMapMove(
        newLocation,
        _mapController.camera.zoom,
      );
    }

    await showModalBottomSheet(
      context: context,
      isScrollControlled: true,
      builder: (BuildContext context) {
        return CreateLocationSheet(
          location: marker,
          newLocation: newLocation);
      },
    );
  }

  void _onZoomIn() {
    _animatedMapMove(
      _mapController.camera.center,
      _mapController.camera.zoom + 1,
    );
  }

  void _onZoomOut() {
    _animatedMapMove(
      _mapController.camera.center,
      _mapController.camera.zoom - 1,
    );
  }

  void _handleMapTap(BuildContext context, LatLng point) {
    _showEditSheet(context, null, newLocation: point);
  }

  void _animatedMapMove(LatLng destLocation, double destZoom) {
    animatedMapMove(destLocation, destZoom, _mapController, this);
  }

  static void animatedMapMove(LatLng destLocation, double destZoom,
      MapController mapController, TickerProvider provider) {
    final latTween = Tween<double>(
        begin: mapController.camera.center.latitude,
        end: destLocation.latitude);
    final lngTween = Tween<double>(
        begin: mapController.camera.center.longitude,
        end: destLocation.longitude);
    final zoomTween =
    Tween<double>(begin: mapController.camera.zoom, end: destZoom);

    final controller = AnimationController(
        duration: const Duration(milliseconds: 500), vsync: provider);

    Animation<double> animation =
    CurvedAnimation(parent: controller, curve: Curves.fastOutSlowIn);

    controller.addListener(() {
      mapController.move(
          LatLng(latTween.evaluate(animation), lngTween.evaluate(animation)),
          zoomTween.evaluate(animation));
    });

    animation.addStatusListener((status) {
      if (status == AnimationStatus.completed) {
        controller.dispose();
      } else if (status == AnimationStatus.dismissed) {
        controller.dispose();
      }
    });

    controller.forward();
  }
}