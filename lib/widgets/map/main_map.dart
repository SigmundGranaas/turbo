import 'package:flutter/material.dart';
import 'package:flutter_map/flutter_map.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:latlong2/latlong.dart';
import 'package:map_app/widgets/map/measuring/measuring_map.dart';
import 'package:map_app/widgets/map/view/map_view_desktop.dart';
import 'package:map_app/widgets/map/view/map_view_mobile.dart';

import '../../data/model/marker.dart' as marker_model;
import '../marker/create_location_sheet.dart';
import 'controller/map_utility.dart';
import 'controls/default_map_controls.dart';
import 'layers/current_location_layer.dart';
import 'layers/tiles/tile_registry/tile_provider.dart';
import 'layers/tiles/tile_registry/tile_registry.dart';
import 'layers/viewport_marker_layer.dart';

class MapControllerPage extends ConsumerStatefulWidget {
  const MapControllerPage({super.key});

  @override
  ConsumerState<MapControllerPage> createState() => MapControllerPageState();
}

class MapControllerPageState extends ConsumerState<MapControllerPage>
    with TickerProviderStateMixin {
  late final MapController _mapController;
  Marker? _temporaryPin;
  final GlobalKey<ScaffoldState> _scaffoldKey = GlobalKey<ScaffoldState>();

  @override
  void initState() {
    super.initState();
    _mapController = MapController();
  }

  @override
  void dispose() {
    _mapController.dispose();
    super.dispose();
  }

  // This function is no longer needed as the search bars will handle animation directly.
  // void _onLocationSelected(double east, double north) {
  //   animatedMapMove(LatLng(north, east), 13, _mapController, this);
  // }

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

    final commonMapLayers = <Widget>[
      ...tileLayers,
      ...attributions,
      const CurrentLocationLayer(),
      ViewportMarkers(mapController: _mapController),
    ];

    return LayoutBuilder(
      builder: (context, constraints) {
        final isMobile = constraints.maxWidth < 600;
        final mapControls = isMobile
            ? defaultMobileMapControls(_mapController, this)
            : defaultMapControls(_mapController, this);

        if (isMobile) {
          return MobileMapView(
            scaffoldKey: _scaffoldKey,
            mapController: _mapController,
            tickerProvider: this,
            mapLayers: commonMapLayers,
            mapControls: mapControls,
            onLongPress: (tap, point) => _handleLongPress(context, point),
            temporaryPin: _temporaryPin,
          );
        } else {
          return DesktopMapView(
            scaffoldKey: _scaffoldKey,
            mapController: _mapController,
            tickerProvider: this,
            mapLayers: commonMapLayers,
            mapControls: mapControls,
            onLongPress: (tap, point) => _handleLongPress(context, point),
            temporaryPin: _temporaryPin,
          );
        }
      },
    );
  }

  // --- Shared Logic Methods ---

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
      animatedMapMove(result.position, _mapController.camera.zoom + 1, _mapController, this);
    }
  }
}