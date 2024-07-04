import 'package:flutter/material.dart';
import 'package:flutter_map/flutter_map.dart';
import 'package:latlong2/latlong.dart';
import 'package:map_app/widgets/map/layers/saved_markers_layer.dart';
import 'package:provider/provider.dart';
import '../marker/create_location_sheet.dart';
import '../../location_provider.dart';
import '../../map/map_layer_button.dart';
import 'package:map_app/data/model/marker.dart' as marker_model;

import '../search.dart';
import 'compass.dart';
import 'layers/tile_providers.dart';

class MapControllerPage extends StatefulWidget {
  static const String route = 'map_controller';

  const MapControllerPage({super.key});

  @override
  MapControllerPageState createState() => MapControllerPageState();
}

class MapControllerPageState extends State<MapControllerPage>
    with TickerProviderStateMixin {
  final GktManager _gktManager = GktManager();
  late final MapController _mapController = MapController();
  String _globalLayer = 'nothing';
  String _norwayLayer = 'topo';

  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addPostFrameCallback((_) {
      context.read<LocationProvider>().loadLocations();
    });
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      body: Consumer<LocationProvider>(
        builder: (context, locationProvider, child) {
          return Stack(
            children: [
               FlutterMap(
                  mapController: _mapController,

                  options: MapOptions(
                    initialCenter: const LatLng(65.0, 13.0), // Center of Norway
                    initialZoom: 5,
                    maxZoom: 20,
                    minZoom: 3,
                    interactionOptions: const InteractionOptions(
                      flags: InteractiveFlag.all,
                      enableMultiFingerGestureRace: true,
                      pinchZoomThreshold: 0.2,
                      pinchMoveThreshold: 40,
                      rotationThreshold: 10.0,
                    ),
                    onTap: (tapPosition, point) =>
                        _handleMapTap(context, point),
                  ),
                  children: [
                    // Use conditional rendering for layers
                    if (_globalLayer == 'osm') openStreetMapTileLayer,
                    if (_norwayLayer == 'topo') norgesKart,
                    if (_norwayLayer == 'satellite') _buildNorgesKartSatelitt(),
                    LocationMarkers(
                        onMarkerTap: (location) =>
                            _showEditSheet(context, location)),
                  ],
                ),
              Positioned(
                top: 120,
                right: 16,
                child: Column(
                  children: [
                    MapLayerButton(
                      currentGlobalLayer: _globalLayer,
                      currentNorwayLayer: _norwayLayer,
                      onBaseLayerChanged: _handleBaseLayerChanged,
                      onNorwayLayerChanged: _handleNorwayLayerChanged,
                    ),
                    const SizedBox(height: 16),
                    CustomCompass(mapController: _mapController),
                    const SizedBox(height: 16),
                    Card(
                      elevation: 4,
                      child: Padding(
                        padding: const EdgeInsets.all(8.0),
                        child: Column(
                          children: [
                            const SizedBox(height: 8),
                            IconButton(
                              icon: const Icon(Icons.add),
                              onPressed: () {
                                _animatedMapMove(
                                  _mapController.camera.center,
                                  _mapController.camera.zoom + 1,
                                );
                              },
                            ),
                            const SizedBox(height: 8),
                            IconButton(
                              icon: const Icon(Icons.remove),
                              onPressed: () {
                                _animatedMapMove(
                                  _mapController.camera.center,
                                  _mapController.camera.zoom - 1,
                                );
                              },
                            ),
                          ],
                        ),
                      ),
                    ),
                  ],
                ),
              ),
              Positioned(
                top: 40,
                left: 0,
                right: 0,
                child: Center(
                  child: SearchWidget(
                    onLocationSelected: (double east, double north) {
                      _animatedMapMove(LatLng(north, east), 13);
                    },
                  ),
                ),
              ),
            ],
          );
        },
      ),
    );
  }

  Widget _buildNorgesKartSatelitt() {
    return FutureBuilder<String>(
      future: _gktManager.getGkt(),
      builder: (context, snapshot) {
        if (snapshot.connectionState == ConnectionState.waiting) {
          return openStreetMapTileLayer;
        } else if (snapshot.hasError) {
          return openStreetMapTileLayer;
        } else if (snapshot.hasData) {
          return TileLayer(
            tileProvider: CustomNorwayTileProvider(snapshot.data!),
          );
        } else {
          return openStreetMapTileLayer;
        }
      },
    );
  }

  void _handleBaseLayerChanged(String layer) {
    setState(() {
      _globalLayer = layer;
    });
  }

  void _handleNorwayLayerChanged(String layer) {
    setState(() {
      _norwayLayer = layer;
    });
  }

  void _handleMapTap(BuildContext context, LatLng point) {
    _showEditSheet(context, null, newLocation: point);
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
        return CreateLocationSheet(newLocation: newLocation);
      },
    );
  }

  void _animatedMapMove(LatLng destLocation, double destZoom) {
    // Tween attributes
    final latTween = Tween<double>(
        begin: _mapController.camera.center.latitude,
        end: destLocation.latitude);
    final lngTween = Tween<double>(
        begin: _mapController.camera.center.longitude,
        end: destLocation.longitude);
    final zoomTween =
        Tween<double>(begin: _mapController.camera.zoom, end: destZoom);

    final controller = AnimationController(
        duration: const Duration(milliseconds: 500), vsync: this);

    Animation<double> animation =
        CurvedAnimation(parent: controller, curve: Curves.fastOutSlowIn);

    // This will make sure the mapController is moved on every tick
    controller.addListener(() {
      _mapController.move(
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
