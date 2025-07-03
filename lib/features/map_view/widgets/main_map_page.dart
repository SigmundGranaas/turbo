import 'package:flutter/material.dart';
import 'package:flutter_map/flutter_map.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:latlong2/latlong.dart';
import 'package:turbo/features/map_view/api.dart';
import 'package:turbo/features/map_view/widgets/layers/current_location_layer.dart';
import 'package:turbo/features/map_view/widgets/layers/viewport_marker_layer.dart';
import 'package:turbo/features/map_view/widgets/view/main_view_desktop.dart';
import 'package:turbo/features/map_view/widgets/view/main_view_mobile.dart';
import 'package:turbo/features/measuring/api.dart';
import 'package:turbo/features/tile_providers/api.dart';
import 'package:turbo/features/tile_storage/offline_regions/api.dart'
as offline_api;
import 'package:turbo/features/tile_storage/offline_regions/widgets/download_progress_toolbar.dart';
import 'package:turbo/l10n/app_localizations.dart';
import 'package:turbo/data/model/marker.dart' as marker_model;

import 'package:turbo/widgets/marker/create_location_sheet.dart';

import '../../../widgets/map/controls/default_map_controls.dart';

class MainMapPage extends ConsumerStatefulWidget {
  const MainMapPage({super.key});

  @override
  ConsumerState<MainMapPage> createState() => _MainMapPageState();
}

class _MainMapPageState extends ConsumerState<MainMapPage>
    with TickerProviderStateMixin {
  late final MapController _mapController;
  Marker? _temporaryPin;
  final GlobalKey<ScaffoldState> _scaffoldKey = GlobalKey<ScaffoldState>();
  final Set<String> _hiddenDownloadIds = {};

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

  void _onMapEvent(MapEvent event) {
    if (event is MapEventMoveEnd ||
        event is MapEventFlingAnimationEnd ||
        event is MapEventRotateEnd) {
      if (mounted) {
        ref.read(mapViewStateProvider.notifier).onMapEvent(event);
      }
    }
  }

  @override
  Widget build(BuildContext context) {
    // Initialize providers and get current state
    final tileRegistryState = ref.watch(tileRegistryProvider);
    final initialMapState = ref.watch(mapViewStateProvider);
    final tileLayers = ref.watch(activeTileLayersProvider);

    // Build attribution widgets for active layers
    final attributions = tileRegistryState.activeGlobalIds
        .followedBy(tileRegistryState.activeLocalIds)
        .map((id) => tileRegistryState.availableProviders[id])
        .whereType<TileProviderConfig>()
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

    final overlayWidgets = <Widget>[];
    final offlineRegionsAsync = ref.watch(offline_api.offlineRegionsProvider);
    final activeDownloads = offlineRegionsAsync.valueOrNull
        ?.where((r) =>
    r.status == offline_api.DownloadStatus.downloading &&
        !_hiddenDownloadIds.contains(r.id))
        .toList() ??
        [];

    if (activeDownloads.isNotEmpty) {
      overlayWidgets.add(
        Positioned(
          bottom: 24,
          left: 16,
          right: 16,
          child: Center(
            child: DownloadProgressToolbar(
              region: activeDownloads.first,
              onHide: () {
                setState(() {
                  _hiddenDownloadIds.add(activeDownloads.first.id);
                });
              },
            ),
          ),
        ),
      );
    }

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
            overlayWidgets: overlayWidgets,
            onLongPress: (tap, point) => _handleLongPress(context, point),
            temporaryPin: _temporaryPin,
            initialCenter: initialMapState.center,
            initialZoom: initialMapState.zoom,
            onMapEvent: _onMapEvent,
          );
        } else {
          return DesktopMapView(
            scaffoldKey: _scaffoldKey,
            mapController: _mapController,
            tickerProvider: this,
            mapLayers: commonMapLayers,
            mapControls: mapControls,
            overlayWidgets: overlayWidgets,
            onLongPress: (tap, point) => _handleLongPress(context, point),
            temporaryPin: _temporaryPin,
            initialCenter: initialMapState.center,
            initialZoom: initialMapState.zoom,
            onMapEvent: _onMapEvent,
          );
        }
      },
    );
  }

  void _handleLongPress(BuildContext context, LatLng point) {
    setState(() {
      _temporaryPin = Marker(
        point: point,
        child: Icon(Icons.location_pin,
            color: Theme.of(context).colorScheme.error, size: 30),
        alignment: Alignment.topCenter,
      );
    });
    _showPinOptionsSheet(context, point);
  }

  void _showPinOptionsSheet(BuildContext context, LatLng point) {
    final l10n = context.l10n;
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
                title: Text(l10n.createNewMarkerHere),
                onTap: () {
                  Navigator.pop(context);
                  _showCreateSheet(context, newLocation: point);
                },
              ),
              ListTile(
                leading: const Icon(Icons.straighten),
                title: Text(l10n.measureDistanceFromHere),
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
    final mapState = ref.read(mapViewStateProvider);
    Navigator.of(context).push(
      MaterialPageRoute(
        builder: (context) => MeasuringMapPage(
          initialPosition: mapState.center,
          startPoint: startPoint,
          zoom: mapState.zoom,
        ),
      ),
    );
  }

  void _showCreateSheet(BuildContext context, {LatLng? newLocation}) async {
    final mapApi = ref.read(mapApiProvider);
    if (newLocation != null) {
      mapApi.animatedMapMove(
          newLocation, _mapController.camera.zoom,
          vsync: this, mapController: _mapController);
    }

    final result = await showModalBottomSheet<marker_model.Marker>(
      context: context,
      isScrollControlled: true,
      builder: (BuildContext context) {
        return CreateLocationSheet(newLocation: newLocation);
      },
    );

    if (result != null && mounted) {
      mapApi.animatedMapMove(
          result.position, _mapController.camera.zoom + 1,
          vsync: this, mapController: _mapController);
    }
  }
}