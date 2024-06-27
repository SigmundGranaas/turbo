import 'package:flutter/material.dart';
import 'package:flutter_map/flutter_map.dart';
import 'package:flutter_map_compass/flutter_map_compass.dart';
import 'package:latlong2/latlong.dart';
import 'package:map_app/saved_markers_layer.dart';
import 'package:map_app/tile_providers.dart';
import 'package:provider/provider.dart';
import 'location_edit_sheet.dart';
import 'location_provider.dart';
import 'map/map_layer_button.dart';
import 'package:map_app/data/model/marker.dart' as marker_model;



class MapControllerPage extends StatefulWidget {
  static const String route = 'map_controller';

  const MapControllerPage({super.key});

  @override
  MapControllerPageState createState() => MapControllerPageState();
}

class MapControllerPageState extends State<MapControllerPage> {
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
      appBar: AppBar(title: const Text('Turbo')),
      body: Consumer<LocationProvider>(
        builder: (context, locationProvider, child){
          return Column(
              children: [
                Flexible(
                  child: FlutterMap(
                    mapController: _mapController,
                    options: MapOptions(
                      initialCenter: const LatLng(65.0, 13.0), // Center of Norway
                      initialZoom: 5,
                      maxZoom: 20,
                      minZoom: 3,
                      onTap: (tapPosition, point) => _handleMapTap(context, point),
                    ),
                    children: [
                      // Use conditional rendering for layers
                      if (_globalLayer == 'osm') openStreetMapTileLayer,
                      if (_norwayLayer == 'topo') norgesKart,
                      if (_norwayLayer == 'satellite') _buildNorgesKartSatelitt(),
                      LocationMarkers(onMarkerTap: (location) => _showEditSheet(context, location)),
                      const MapCompass.cupertino()
                    ],
                  ),
                ),
              ],
          );
        },
    ),
      floatingActionButton: MapLayerButton(
        currentGlobalLayer: _globalLayer,
        currentNorwayLayer: _norwayLayer,
        onBaseLayerChanged: _handleBaseLayerChanged,
        onNorwayLayerChanged: _handleNorwayLayerChanged,
      ),
    );
  }

  Widget _buildNorgesKartSatelitt() {
    return FutureBuilder<String>(
      future: _gktManager.getGkt(),
      builder: (context, snapshot) {
        if (snapshot.connectionState == ConnectionState.waiting) {
          return TileLayer(
            urlTemplate: 'https://tile.openstreetmap.org/{z}/{x}/{y}.png',
            userAgentPackageName: 'com.example.app',
          );
        } else if (snapshot.hasError) {
          return TileLayer(
            urlTemplate: 'https://tile.openstreetmap.org/{z}/{x}/{y}.png',
            userAgentPackageName: 'com.example.app',
          );
        } else if (snapshot.hasData) {
          return TileLayer(
            tileProvider: CustomNorwayTileProvider(snapshot.data!),
          );
        } else {
          return TileLayer(
            urlTemplate: 'https://tile.openstreetmap.org/{z}/{x}/{y}.png',
            userAgentPackageName: 'com.example.app',
          );
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

  void _showEditSheet(BuildContext context, marker_model.Marker? marker, {LatLng? newLocation}) {
    showModalBottomSheet(
      context: context,
      isScrollControlled: true,
      showDragHandle: true,
      builder: (context) => LocationEditSheet(location: marker, newLocation: newLocation),
    );
  }
}