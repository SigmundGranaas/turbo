import 'package:flutter/material.dart';
import 'package:flutter_map/flutter_map.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:latlong2/latlong.dart';
import 'package:turbo/features/map_view/api.dart';
import 'package:turbo/features/map_view/widgets/layers/current_location_layer.dart';
import 'package:turbo/features/map_view/widgets/layers/viewport_marker_layer.dart';
import 'package:turbo/features/activities/api.dart' as activities;
import 'package:turbo/features/curated_paths/api.dart';

import 'package:turbo/features/external_vector_layers/api.dart';
import 'package:turbo/features/saved_paths/api.dart';
import 'package:turbo/features/map_view/widgets/view/main_view_desktop.dart';
import 'package:turbo/features/map_view/widgets/view/main_view_mobile.dart';
import 'package:turbo/features/measuring/api.dart';
import 'package:turbo/features/routing/api.dart';
import 'package:turbo/features/path_recording/api.dart';
import 'package:turbo/features/tile_providers/api.dart';
import 'package:turbo/features/tile_storage/offline_regions/api.dart'
as offline_api;
import 'package:turbo/app/l10n/app_localizations.dart';
import 'package:turbo/features/markers/api.dart' as marker_model;
import 'package:turbo/features/navigation/api.dart';
import 'package:turbo/features/photo_map/api.dart';
import 'package:turbo/features/sharing/api.dart';
import 'package:turbo/features/weather/api.dart' show OceanConditionsLayer;

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
        // Manual drag pauses follow rather than disabling it — the user's
        // intent (snap-to-me) is preserved and a single tap on the location
        // button resumes. The mode chip's close button still fully disables.
        ref.read(followModeProvider.notifier).pause();
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
      if (position != null && ref.read(followModeProvider).isOn) {
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

    final activeOverlayIds = tileRegistryState.activeOverlayIds.toSet();
    final trailVectorLayers = <Widget>[
      for (final entry in trailOverlayIdToSubtype.entries)
        VectorDataLayer(
          source: trailVectorSource(entry.value),
          mapController: _mapController,
          visible: activeOverlayIds.contains(entry.key),
        ),
      // OSM Overpass paths and Kartverket N50 Sti — additional vector
      // sources, toggled via their own overlay configs (vector-only,
      // see `vector_path_overlays.dart`). They render alongside the
      // Turrutebasen layers and reuse the same TrailFeatureSheet on tap.
      VectorDataLayer(
        source: osmPathVectorSource(),
        mapController: _mapController,
        visible: activeOverlayIds.contains('osm_paths'),
      ),
      VectorDataLayer(
        source: n50StiVectorSource(),
        mapController: _mapController,
        visible: activeOverlayIds.contains('n50_sti'),
      ),
      // Curated MVT overlays served by the Turbo tileserver
      // (apps/tileserver). Stylistically parallel to the existing
      // GeoJSON-backed overlays above, but use MvtDataLayer which
      // fetches per-tile from `/v1/{resource}/tiles/{z}/{x}/{y}.mvt`.
      for (final entry in ref.watch(curatedSourcesByIdProvider).entries)
        MvtDataLayer(
          source: entry.value,
          mapController: _mapController,
          visible: activeOverlayIds.contains(entry.key),
        ),
    ];

    final commonMapLayers = <Widget>[
      ...tileLayers,
      ...attributions,
      // Recording trace renders below the location marker so the dot stays
      // visually on top of its own track.
      const RecordingTraceLayer(),
      const CurrentLocationLayer(),
      const NavigationPolylineLayer(),
      const NavigationTargetMarker(),
      SavedPathsLayer(mapController: _mapController),
      ...trailVectorLayers,
      OceanConditionsLayer(
        mapController: _mapController,
        visible: activeOverlayIds.contains('ocean_conditions'),
      ),
      ViewportMarkers(mapController: _mapController),
      const activities.ActivitiesMapLayer(),
      PhotoMapLayer(mapController: _mapController),
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
        left: 0,
        right: 0,
        bottom: 20,
        child: Center(child: RecordingPanel()),
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

    return SharedLinkRedemptionListener(
      child: SharedPayloadListener(
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
    // Centre on the pin so it sits in the middle of the top half of the
    // screen — the sheet covers the bottom half. Shifting the camera
    // target south by 25 % of the visible latitudinal span puts the pin
    // exactly at 25 % from the top edge of the viewport.
    final camera = _mapController.camera;
    final latSpan = camera.visibleBounds.north - camera.visibleBounds.south;
    final target = LatLng(point.latitude - latSpan * 0.25, point.longitude);
    animatedMapMove(target, camera.zoom, _mapController, this);
    _showPinOptionsSheet(context, point);
  }

  void _showPinOptionsSheet(BuildContext context, LatLng point) {
    final isNavigating = ref.read(navigationStateProvider).isActive;
    showModalBottomSheet(
      context: context,
      isScrollControlled: true,
      useSafeArea: true,
      backgroundColor: Colors.transparent,
      builder: (BuildContext sheetContext) {
        return PinOptionsSheet(
          point: point,
          isNavigating: isNavigating,
          onCreateMarker: (namePreview) => _showCreateSheet(
            context,
            newLocation: point,
            prefillName: namePreview,
          ),
          onCreateActivity: () => _showActivityCreatePicker(context, point),
          onMeasure: () => _navigateToMeasuring(point),
          onPlanRoute: () => _navigateToRoutePlanning([point]),
          onNavigate: () => _routeFromMyLocation(point),
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

  /// Open the cross-kind activity picker seeded at the tapped point.
  /// The picker dispatches to the selected kind's create screen via its
  /// descriptor — the map page stays unaware of which kinds exist.
  /// The picker itself renders a sign-in CTA for anonymous users so we
  /// don't need an auth check here.
  void _showActivityCreatePicker(BuildContext context, LatLng point) {
    showModalBottomSheet(
      context: context,
      isScrollControlled: true,
      useSafeArea: true,
      builder: (sheetCtx) => activities.ActivityCreatePicker.fromPoint(point),
    );
  }

  void _navigateToRoutePlanning(List<LatLng> seeds) {
    Navigator.of(context).push<void>(
      MaterialPageRoute(
        builder: (context) => RoutePlanningPage(
          initialCenter: _mapController.camera.center,
          initialZoom: _mapController.camera.zoom,
          initialWaypoints: seeds,
        ),
      ),
    );
  }

  /// "Navigate to here" → plot an actual route from the user's current
  /// location to the tapped point (not a straight line). Falls back to
  /// seeding just the destination if location isn't available yet.
  void _routeFromMyLocation(LatLng destination) {
    final me = ref.read(locationStateProvider).value;
    if (me == null) {
      AppSnackbars.info(
          context, 'Location unavailable — tap the map to set your start');
    }
    _navigateToRoutePlanning(me != null ? [me, destination] : [destination]);
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

  void _showCreateSheet(
    BuildContext context, {
    LatLng? newLocation,
    String? prefillName,
  }) async {
    if (newLocation != null) {
      animatedMapMove(
          newLocation, _mapController.camera.zoom, _mapController, this);
    }

    if (!context.mounted) return;
    final result = await showModalBottomSheet<marker_model.Marker>(
      context: context,
      isScrollControlled: true,
      useSafeArea: true,
      builder: (BuildContext context) {
        return marker_model.CreateLocationSheet(
          newLocation: newLocation,
          initialName: prefillName,
        );
      },
    );

    if (result != null && mounted) {
      animatedMapMove(
          result.position, _mapController.camera.zoom + 1, _mapController, this);
    }
  }
}