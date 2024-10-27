import 'package:flutter/material.dart';
import 'package:flutter_map/flutter_map.dart';
import 'package:latlong2/latlong.dart';
import 'package:map_app/widgets/map/compass.dart';
import 'package:map_app/widgets/map/layers/current_location_layer.dart';
import 'package:map_app/widgets/map/layers/saved_markers_layer.dart';
import 'package:map_app/widgets/map/plus_minus_buttons.dart';
import 'package:provider/provider.dart';
import '../marker/create_location_sheet.dart';
import '../../location_provider.dart';
import 'layers/map_layer_button.dart';
import 'package:map_app/data/model/marker.dart' as marker_model;

import 'layers/tile_providers.dart';
import 'location_button.dart';

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
                  // Use conditional rendering for layers
                  if (_globalLayer == 'osm') _buildOsm(),
                  if (_globalLayer == 'gs') _buildGoogleSatellite(),
                  if (_norwayLayer == 'topo') _buildTopo(),
                  if (_norwayLayer == 'satellite') _buildNorgesKartSatelitt(),
                  const CurrentLocationLayer(),
                  LocationMarkers(
                      onMarkerTap: (location) =>
                          _showEditSheet(context, location)),
                ],
              ),
              Positioned(
                top: 80,
                right: 16,
                child: Column(
                  children: [
                    MapLayerButton(
                      currentGlobalLayer: _globalLayer,
                      currentNorwayLayer: _norwayLayer,
                      onBaseLayerChanged: _handleBaseLayerChanged,
                      onNorwayLayerChanged: _handleNorwayLayerChanged,
                    ),
                    LocationButton(mapController: _mapController),
                    CustomMapCompass(mapController: _mapController),
                    PlusMinusButtons(onZoomIn: _onZoomIn, onZoomOut: _onZoomOut)
                  ],
                ),
              ),
            ],
          );
        },
      ),
    );
  }

  void _onZoomIn(){
    _animatedMapMove(
      _mapController.camera.center,
      _mapController.camera.zoom + 1,
    );
  }
  void _onZoomOut(){
    _animatedMapMove(
      _mapController.camera.center,
      _mapController.camera.zoom - 1,
    );
  }

  Widget _buildTopo() {
    return FutureBuilder<String?>(
      future: getPath(),
      builder: (context, snapshot) {
          if(snapshot.connectionState == ConnectionState.done){
            return norgesKart(snapshot.data);
          } else {
            return _buildOsm();
          }
        },
    );
  }
  Widget _buildOsm() {
    return FutureBuilder<String?>(
      future: getPath(),
      builder: (context, snapshot) {
        if(snapshot.connectionState == ConnectionState.done){
          return openStreetMap(snapshot.data);
        } else {
          return const Stack();
        }
      },
    );
  }

  Widget _buildGoogleSatellite() {
    return FutureBuilder<String?>(
      future: getPath(),
      builder: (context, snapshot) {
        if(snapshot.connectionState == ConnectionState.done){
          return googleSatellite(snapshot.data);
        } else {
          return const Stack();
        }
      },
    );
  }

  Widget _buildNorgesKartSatelitt() {
    return FutureBuilder<(String, String?)>(
      future: getPathAndGkt(),
      builder: (context, snapshot) {
        if (snapshot.connectionState == ConnectionState.waiting) {
          return _buildOsm();
        } else if (snapshot.hasError) {
          return _buildOsm();
        } else if (snapshot.hasData) {
          return norgeSatelitt(snapshot.data!);
        } else {
          return _buildOsm();
        }
      },
    );
  }

  Future<(String, String?)> getPathAndGkt(){
    return _gktManager.getGkt().then((gkt) => getPath().then((path) => Future.value((gkt, path))));
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
     animatedMapMove(destLocation, destZoom, _mapController, this);
   }

    static void animatedMapMove(LatLng destLocation, double destZoom, MapController mapController, TickerProvider provider) {
    // Tween attributes
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

    // This will make sure the mapController is moved on every tick
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
