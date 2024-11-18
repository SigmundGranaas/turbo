import 'package:flutter/material.dart';
import 'package:flutter_map/flutter_map.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:latlong2/latlong.dart';
import 'package:map_app/data/search/marker_search_service.dart';
import 'package:map_app/widgets/map/measuring/measuring_map.dart';

import '../../data/datastore/factory.dart';
import '../../data/model/marker.dart' as marker_model;
import '../../data/search/composite_search_service.dart';
import '../../data/search/kartverket_location_service.dart';
import '../marker/create_location_sheet.dart';
import '../search/searchbar.dart';
import 'controller/map_utility.dart';
import 'controller/provider/map_controller.dart';
import 'controls/default_map_controls.dart';
import 'controls/map_controls.dart';
import 'layers/current_location_layer.dart';
import 'layers/saved_markers_layer.dart';
import 'layers/tiles/tile_registry/tile_registry.dart';
import 'map_base.dart';

class MapControllerPage extends ConsumerStatefulWidget {
  const MapControllerPage({super.key});

  @override
  ConsumerState<MapControllerPage> createState() => MapControllerPageState();
}

class MapControllerPageState extends ConsumerState<MapControllerPage>
    with TickerProviderStateMixin {
  late MapController _mapController;
  Marker? _temporaryPin;

  @override
  Widget build(BuildContext context) {
    final layers = ref.watch(tileRegistryProvider.notifier).getActiveLayers();
    final registry = ref.watch(tileRegistryProvider);
    _mapController = ref.watch(mapControllerProvProvider.notifier).controller();

    final mapLayers = [
      ...layers,
      ...registry.activeGlobalIds.map((id) =>
          RichAttributionWidget(
            animationConfig: const ScaleRAWA(),
            attributions: [
              TextSourceAttribution(
                  registry.availableProviders[id]!.attributions),
            ],
          )),
      ...registry.activeLocalIds.map((id) =>
          RichAttributionWidget(
            animationConfig: const ScaleRAWA(),
            attributions: [
              TextSourceAttribution(
                  registry.availableProviders[id]!.attributions),
            ],
          )),
      const CurrentLocationLayer(),
      LocationMarkers(
        onMarkerTap: (location) => _showEditSheet(context, location),
      ),
      if (_temporaryPin != null) MarkerLayer(markers: [_temporaryPin!]),
    ];

    final width = MediaQuery.of(context).size.width;
    final isMobile = width < 600;
    final List<Widget> controls;

    if(isMobile){
      controls = defaultMobileMapControls(_mapController, this);
    }else{
      controls = defaultMapControls(_mapController, this);
    }

    return Scaffold(
      body: MapBase(
        mapController: _mapController,
        mapLayers: mapLayers,
        overlayWidgets: [
          MapControls(controls: controls),
          Positioned(
            top: 20,
            left: 0,
            right: 0,
            child: Center(
              child: SearchWidget(
                  onLocationSelected: (double east, double north) {
                    animatedMapMove(LatLng(north, east), 13, _mapController, this);
                  },
                  service: CompositeSearchService(KartverketLocationService(), MarkerSearchService(MarkerDataStoreFactory.getDataStore()))),
            ),
          ),
        ],
        onLongPress: (tapPosition, point) => _handleLongPress(context, point),
      ),
    );
  }
    void _handleLongPress(BuildContext context, LatLng point) {
      setState(() {
        _temporaryPin = Marker(
          point: point,
          child: const Icon(Icons.location_pin, color: Colors.red, size: 30),
        );
      });

      _showPinOptionsSheet(context, point);
    }

  void _showPinOptionsSheet(BuildContext context, LatLng point) {
    showModalBottomSheet(
      context: context,
      builder: (BuildContext context) {
        return Container(
          padding: const EdgeInsets.all(16),
          child: Column(
            mainAxisSize: MainAxisSize.min,
            children: [
              ListTile(
                leading: const Icon(Icons.save),
                title: const Text('Save Location'),
                onTap: () {
                  Navigator.pop(context);
                  _showEditSheet(context, null, newLocation: point);
                },
              ),
              ListTile(
                leading: const Icon(Icons.straighten),
                title: const Text('Start Measuring'),
                onTap: () {
                  Navigator.pop(context);
                  _navigateToMeasuring(point);
                },
              ),
            ],
          ),
        );
      },
    ).whenComplete(() {
      setState(() {
        _temporaryPin = null;
      });
    });
  }

  void _navigateToMeasuring(LatLng startPoint) {
    Navigator.of(context).push(
      MaterialPageRoute(
        builder: (context) => MeasuringControllerPage(
          initialPosition: _mapController.camera.center,
          startPoint: startPoint,
          zoom: _mapController.camera.zoom,
        ),
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
  }
