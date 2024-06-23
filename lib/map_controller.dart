import 'package:dio/dio.dart';
import 'package:dio_cache_interceptor/dio_cache_interceptor.dart';
import 'package:flutter/material.dart';
import 'package:flutter_map/flutter_map.dart';
import 'package:flutter_map_compass/flutter_map_compass.dart';
import 'package:latlong2/latlong.dart';
import 'package:map_app/saved_markers_layer.dart';
import 'package:map_app/tile_providers.dart';
import 'package:provider/provider.dart';
import 'location_edit_sheet.dart';
import 'location_provider.dart';


class MapControllerPage extends StatefulWidget {
  static const String route = 'map_controller';

  const MapControllerPage({super.key});

  @override
  MapControllerPageState createState() => MapControllerPageState();
}

class MapControllerPageState extends State<MapControllerPage> {
  final CacheStore _cacheStore = MemCacheStore();
  final _dio = Dio();
  late final MapController _mapController = MapController();

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
      appBar: AppBar(title: const Text('MapController')),
      body: Consumer<LocationProvider>(
        builder: (context, locationProvider, child){
          return Column(
              children: [
                Flexible(
                  child: FlutterMap(
                    mapController: _mapController,
                    options: MapOptions(
                      initialCenter: const  LatLng(65.0, 13.0), // Center of Norway
                      initialZoom: 5,
                      maxZoom: 20,
                      minZoom: 3,
                      onTap: (tapPosition, point) => _handleMapTap(context, point),
                    ),
                    children: [
                      openStreetMapTileLayer,
                      norgesKart,
                      LocationMarkers(onMarkerTap: (location) => _showEditSheet(context, location)),
                      const MapCompass.cupertino()
                    ],
                  ),
                ),
              ],
          );
        },
      ),
    );
  }

  void _handleMapTap(BuildContext context, LatLng point) {
    _showEditSheet(context, null, newLocation: point);
  }

  void _showEditSheet(BuildContext context, Map<String, dynamic>? location, {LatLng? newLocation}) {
    showModalBottomSheet(
      context: context,
      isScrollControlled: true,
      showDragHandle: true,
      builder: (context) => LocationEditSheet(location: location, newLocation: newLocation),
    );
  }
}