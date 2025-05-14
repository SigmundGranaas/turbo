import 'package:flutter/material.dart';
import 'package:flutter_map/flutter_map.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:latlong2/latlong.dart';
import 'package:map_app/widgets/auth/drawer_widget.dart';
import 'package:map_app/widgets/map/measuring/measuring_map.dart';

import '../../data/model/marker.dart' as marker_model;
import '../../data/search/composite_search_service.dart';
import '../marker/create_location_sheet.dart';
import '../search/searchbar.dart';
import 'controller/map_utility.dart';
import 'controller/provider/map_controller.dart';
import 'controls/default_map_controls.dart';
import 'controls/map_controls.dart';
import 'layers/current_location_layer.dart';
import 'layers/tiles/tile_registry/tile_provider.dart';
import 'layers/tiles/tile_registry/tile_registry.dart';
import 'layers/viewport_marker_layer.dart';
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
  final GlobalKey<ScaffoldState> _scaffoldKey = GlobalKey<ScaffoldState>();

  @override
  void initState() {
    super.initState();
    _mapController = ref.read(mapControllerProvProvider.notifier).controller();
    // No need to init localMarkerDataStoreProvider here, LocationRepository handles it.
  }

  @override
  Widget build(BuildContext context) {
    final activeTileLayers = ref.watch(tileRegistryProvider.select((s) => s.activeGlobalIds + s.activeLocalIds + s.activeOverlayIds));
    final availableProviders = ref.watch(tileRegistryProvider.select((s) => s.availableProviders));

    List<TileLayer> tileLayers = activeTileLayers
        .map((id) => availableProviders[id]?.createTileLayer())
        .whereType<TileLayer>()
        .toList();

    List<RichAttributionWidget> attributions = activeTileLayers
        .map((id) => availableProviders[id])
        .whereType<TileProviderWrapper>()
        .map((provider) => RichAttributionWidget(
      animationConfig: const ScaleRAWA(),
      attributions: [TextSourceAttribution(provider.attributions)],
    ))
        .toList();

    final mapLayers = <Widget>[
      ...tileLayers,
      ...attributions,
      const CurrentLocationLayer(),
      ViewportMarkers(mapController: _mapController),
      if (_temporaryPin != null) MarkerLayer(markers: [_temporaryPin!]),
    ];

    final width = MediaQuery.of(context).size.width;
    final isMobile = width < 600;
    final List<Widget> controls = isMobile
        ? defaultMobileMapControls(_mapController, this)
        : defaultMapControls(_mapController, this);

    final colorScheme = Theme.of(context).colorScheme;

    return Scaffold(
      key: _scaffoldKey,
      drawer: const AppDrawer(),
      body: Stack(
        children: [
          MapBase(
            mapController: _mapController,
            mapLayers: mapLayers,
            overlayWidgets: [
              MapControls(controls: controls),
              Positioned(
                top: 20,
                left: 0,
                right: 0,
                child: Center(
                  child: _buildCustomSearchBar(isMobile),
                ),
              ),
            ],
            onLongPress: (tapPosition, point) => _handleLongPress(context, point),
          ),
          if (!isMobile)
            Positioned(
              left: 20,
              top: 20,
              child: SizedBox(
                width: 64,
                height: 64,
                child: Card(
                  elevation: 4,
                  shape: const CircleBorder(),
                  child: ClipOval(
                    child: Material(
                      color: Theme.of(context).colorScheme.surface,
                      child: InkWell(
                        onTap: () => _scaffoldKey.currentState?.openDrawer(),
                        child: Padding(
                          padding: const EdgeInsets.all(8),
                          child: Icon(Icons.menu, color: colorScheme.primary),
                        ),
                      ),
                    ),
                  ),
                ),
              ),
            ),
        ],
      ),
    );
  }

  Widget _buildCustomSearchBar(bool isMobile) {
    final searchService = ref.watch(compositeSearchServiceProvider);

    return SearchWidget(
      onLocationSelected: (double east, double north) {
        animatedMapMove(LatLng(north, east), 13, _mapController, this);
      },
      service: searchService,
    );
  }

  void _handleLongPress(BuildContext context, LatLng point) {
    setState(() {
      _temporaryPin = Marker(
        point: point,
        child: const Icon(Icons.location_pin, color: Colors.red, size: 30),
        alignment: Alignment.topCenter,
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
                leading: const Icon(Icons.add_location_alt_outlined),
                title: const Text('Create New Marker Here'),
                onTap: () {
                  Navigator.pop(context);
                  _showCreateSheet(context, newLocation: point);
                },
              ),
              ListTile(
                leading: const Icon(Icons.straighten),
                title: const Text('Measure Distance From Here'),
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
      if (mounted) {
        setState(() => _temporaryPin = null);
      }
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

  void _showCreateSheet(BuildContext context, {LatLng? newLocation}) async {
    if (newLocation != null) {
      animatedMapMove(newLocation, _mapController.camera.zoom, _mapController, this);
    }

    final result = await showModalBottomSheet<marker_model.Marker>(
      context: context,
      isScrollControlled: true,
      builder: (BuildContext context) {
        return CreateLocationSheet(newLocation: newLocation);
      },
    );

    if (result != null && mounted) {
      animatedMapMove(result.position, _mapController.camera.zoom +1, _mapController, this);
    }
  }
}