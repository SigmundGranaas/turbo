import 'package:flutter/material.dart';
import 'package:flutter_map/flutter_map.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:latlong2/latlong.dart';
import 'package:map_app/widgets/map/buttons/plus_minus_buttons.dart';
import 'package:map_app/widgets/map/controller/map_utility.dart';

import '../../data/model/marker.dart' as marker_model;
import '../marker/create_location_sheet.dart';
import 'buttons/compass.dart';
import 'controller/provider/map_controller.dart';
import 'layers/current_location_layer.dart';
import 'buttons/map_layer_button.dart';
import 'layers/saved_markers_layer.dart';
import 'layers/tiles/tile_registry/tile_registry.dart';
import 'buttons/location_button.dart';

class MapControllerPage extends ConsumerStatefulWidget {

  const MapControllerPage({super.key});

  @override
  ConsumerState<MapControllerPage> createState() => MapControllerPageState();
}

class MapControllerPageState extends ConsumerState<MapControllerPage>
    with TickerProviderStateMixin {
  late MapController _mapController;

  @override
  void initState() {
    super.initState();
  }

  @override
  Widget build(BuildContext context) {
    final layers = ref.watch(tileRegistryProvider.notifier).getActiveLayers();
    final registry = ref.watch(tileRegistryProvider);
    _mapController = ref.watch(mapControllerProvProvider.notifier).controller();

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
                const LocationButton(),
                CustomMapCompass(mapController: _mapController),
                PlusMinusButtons(
                  onZoomIn: () => zoomIn(_mapController, this),
                  onZoomOut: () => zoomOut(_mapController, this),
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
      animatedMapMove(
        newLocation,
        _mapController.camera.zoom,
        _mapController,
        this
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

  void _handleMapTap(BuildContext context, LatLng point) {
    _showEditSheet(context, null, newLocation: point);
  }
}