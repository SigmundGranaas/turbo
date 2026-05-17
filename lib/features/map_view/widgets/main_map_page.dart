import 'package:flutter/material.dart';
import 'package:flutter_map/flutter_map.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:latlong2/latlong.dart';
import 'package:turbo/features/map_view/api.dart';
import 'package:turbo/features/map_view/widgets/layers/current_location_layer.dart';
import 'package:turbo/features/map_view/widgets/layers/viewport_marker_layer.dart';
import 'package:turbo/features/saved_paths/api.dart';
import 'package:turbo/features/map_view/widgets/view/main_view_desktop.dart';
import 'package:turbo/features/map_view/widgets/view/main_view_mobile.dart';
import 'package:turbo/features/measuring/api.dart';
import 'package:turbo/features/path_recording/api.dart';
import 'package:turbo/features/tile_providers/api.dart';
import 'package:turbo/features/tile_storage/offline_regions/api.dart'
as offline_api;
import 'package:turbo/app/l10n/app_localizations.dart';
import 'package:turbo/features/markers/api.dart' as marker_model;
import 'package:turbo/features/navigation/api.dart';
import 'package:turbo/features/sharing/api.dart';

import 'package:turbo/core/location/compass_mode_state.dart';
import 'package:turbo/core/location/compass_state.dart';
import 'package:turbo/core/location/follow_mode_state.dart';
import 'package:turbo/core/location/location_state.dart';
import 'package:turbo/core/widgets/map/controller/map_utility.dart';
import 'package:turbo/core/widgets/map/controls/default_map_controls.dart';
import 'package:turbo/features/map_view/widgets/mode_indicator.dart';
import 'package:turbo/features/map_view/widgets/pin_options_sheet.dart';
import 'package:turbo/core/widgets/app_snackbars.dart';

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

  AnimationController? _followAnimController;
  AnimationController? _compassAnimController;

  @override
  void initState() {
    super.initState();
    _mapController = MapController();
  }

  @override
  void dispose() {
    _followAnimController?.dispose();
    _compassAnimController?.dispose();
    _mapController.dispose();
    super.dispose();
  }

  void _smoothMoveTo(LatLng position) {
    _followAnimController?.stop();
    _followAnimController?.dispose();

    _followAnimController = AnimationController(
      duration: const Duration(milliseconds: 300),
      vsync: this,
    );

    final latTween = Tween<double>(
      begin: _mapController.camera.center.latitude,
      end: position.latitude,
    );
    final lngTween = Tween<double>(
      begin: _mapController.camera.center.longitude,
      end: position.longitude,
    );

    final curved = CurvedAnimation(
      parent: _followAnimController!,
      curve: Curves.easeOutCubic,
    );

    _followAnimController!.addListener(() {
      _mapController.move(
        LatLng(latTween.evaluate(curved), lngTween.evaluate(curved)),
        _mapController.camera.zoom,
      );
    });

    _followAnimController!.addStatusListener((status) {
      if (status == AnimationStatus.completed ||
          status == AnimationStatus.dismissed) {
        _followAnimController?.dispose();
        _followAnimController = null;
      }
    });

    _followAnimController!.forward();
  }

  void _smoothRotateTo(double targetDegrees) {
    _compassAnimController?.stop();
    _compassAnimController?.dispose();

    _compassAnimController = AnimationController(
      duration: const Duration(milliseconds: 200),
      vsync: this,
    );

    final currentRotation = _mapController.camera.rotation;
    // Normalize to shortest rotation path
    double diff = (targetDegrees - currentRotation) % 360;
    if (diff > 180) diff -= 360;
    if (diff < -180) diff += 360;
    final normalizedTarget = currentRotation + diff;

    final rotationTween = Tween<double>(
      begin: currentRotation,
      end: normalizedTarget,
    );

    final curved = CurvedAnimation(
      parent: _compassAnimController!,
      curve: Curves.easeOutCubic,
    );

    _compassAnimController!.addListener(() {
      _mapController.rotate(rotationTween.evaluate(curved));
    });

    _compassAnimController!.addStatusListener((status) {
      if (status == AnimationStatus.completed ||
          status == AnimationStatus.dismissed) {
        _compassAnimController?.dispose();
        _compassAnimController = null;
      }
    });

    _compassAnimController!.forward();
  }

  void _onMapEvent(MapEvent event) {
    if (event is MapEventMoveEnd ||
        event is MapEventFlingAnimationEnd ||
        event is MapEventRotateEnd) {
      if (mounted) {
        ref.read(mapViewStateProvider.notifier).onMapEvent(event);
      }
    }

    // Disengage follow mode on user-initiated pan/drag/fling
    if (event.source == MapEventSource.onDrag ||
        event.source == MapEventSource.onMultiFinger ||
        event.source == MapEventSource.flingAnimationController) {
      if (event is MapEventWithMove || event is MapEventMoveStart) {
        ref.read(followModeProvider.notifier).disable();
      }
    }

    // Disengage compass mode on user-initiated rotation
    if (event is MapEventRotate || event is MapEventRotateStart) {
      if (event.source == MapEventSource.onMultiFinger ||
          event.source == MapEventSource.cursorKeyboardRotation) {
        ref.read(compassModeProvider.notifier).disable();
      }
    }
  }

  @override
  Widget build(BuildContext context) {
    // Follow mode: smoothly move map when location updates
    ref.listen<AsyncValue<LatLng?>>(locationStateProvider, (previous, next) {
      final position = next.value;
      if (position != null && ref.read(followModeProvider)) {
        _smoothMoveTo(position);
      }
    });

    // Compass mode: smoothly rotate map when compass heading updates
    ref.listen<AsyncValue<double?>>(compassStateProvider, (previous, next) {
      final heading = next.value;
      if (heading != null && ref.read(compassModeProvider)) {
        _smoothRotateTo(-heading);
      }
    });

    // Navigation arrival detection: auto-cancel when within 15m of target
    ref.listen<AsyncValue<LatLng?>>(locationStateProvider, (previous, next) {
      final position = next.value;
      final navState = ref.read(navigationStateProvider);
      if (position != null && navState.isActive && navState.target != null) {
        final distance = const Distance().distance(position, navState.target!);
        if (distance < 15) {
          ref.read(navigationStateProvider.notifier).stopNavigation();
          if (mounted) {
            AppSnackbars.info(context, context.l10n.youHaveArrived);
          }
        }
      }
    });

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
      const NavigationPolylineLayer(),
      const NavigationTargetMarker(),
      SavedPathsLayer(mapController: _mapController),
      ViewportMarkers(mapController: _mapController),
    ];

    final overlayWidgets = <Widget>[
      const ModeIndicator(),
      const Positioned(
        left: 0,
        right: 0,
        bottom: 0,
        child: marker_model.MarkerSelectionBar(),
      ),
      const Positioned(
        top: 56,
        left: 0,
        right: 0,
        child: RecordingHud(),
      ),
      const Positioned(
        right: 16,
        bottom: 96,
        child: RecordingFab(),
      ),
    ];
    final offlineRegionsAsync = ref.watch(offline_api.offlineRegionsProvider);
    final activeDownloads = offlineRegionsAsync.value
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
            child: offline_api.DownloadProgressToolbar(
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

    return SharedPayloadListener(
      onCenter: (point) => _smoothMoveTo(point),
      child: LayoutBuilder(
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
      ),
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
    final isNavigating = ref.read(navigationStateProvider).isActive;
    showModalBottomSheet(
      context: context,
      useSafeArea: true,
      builder: (BuildContext sheetContext) {
        return PinOptionsSheet(
          isNavigating: isNavigating,
          onCreateMarker: () => _showCreateSheet(context, newLocation: point),
          onMeasure: () => _navigateToMeasuring(point),
          onNavigate: () => ref
              .read(navigationStateProvider.notifier)
              .startNavigation(point),
          onStopNavigation: () =>
              ref.read(navigationStateProvider.notifier).stopNavigation(),
        );
      },
    ).whenComplete(() {
      if (mounted) {
        setState(() => _temporaryPin = null);
      }
    });
  }

  void _navigateToMeasuring(LatLng startPoint) async {
    final result = await Navigator.of(context).push<bool>(
      MaterialPageRoute(
        builder: (context) => MeasuringMapPage(
          initialPosition: _mapController.camera.center,
          zoom: _mapController.camera.zoom,
        ),
      ),
    );
    if (result == true && mounted) {
      AppSnackbars.success(context, context.l10n.pathSaved);
    }
  }

  void _showCreateSheet(BuildContext context, {LatLng? newLocation}) async {
    if (newLocation != null) {
      animatedMapMove(
          newLocation, _mapController.camera.zoom, _mapController, this);
    }

    final result = await showModalBottomSheet<marker_model.Marker>(
      context: context,
      isScrollControlled: true,
      useSafeArea: true,
      builder: (BuildContext context) {
        return marker_model.CreateLocationSheet(newLocation: newLocation);
      },
    );

    if (result != null && mounted) {
      animatedMapMove(
          result.position, _mapController.camera.zoom + 1, _mapController, this);
    }
  }
}