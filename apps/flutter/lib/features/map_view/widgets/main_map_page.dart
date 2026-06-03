import 'package:flutter/material.dart';
import 'package:flutter_map/flutter_map.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:latlong2/latlong.dart';
import 'package:turbo/features/map_view/api.dart';
import 'package:turbo/features/activities/api.dart' as activities;
import 'package:turbo/features/map_view/widgets/view/main_view_desktop.dart';
import 'package:turbo/features/map_view/widgets/view/main_view_mobile.dart';
import 'package:turbo/features/measuring/api.dart';
import 'package:turbo/features/routing/api.dart';
import 'package:turbo/features/journey/api.dart';
import 'package:turbo/features/tile_providers/api.dart';
import 'package:turbo/app/l10n/app_localizations.dart';
import 'package:turbo/features/markers/api.dart' as marker_model;
import 'package:turbo/features/navigation/api.dart';
import 'package:turbo/features/search/api.dart' show LocationSearchResult;
import 'package:turbo/features/sharing/api.dart';

import 'package:turbo/core/location/compass_mode_state.dart';
import 'package:turbo/core/location/compass_state.dart';
import 'package:turbo/core/location/follow_mode_state.dart';
import 'package:turbo/core/location/location_state.dart';
import 'package:turbo/core/widgets/map/controller/map_utility.dart';
import 'package:turbo/core/widgets/map/controls/default_map_controls.dart';
import 'package:turbo/core/widgets/map/map_overlay_host.dart';
import 'package:turbo/core/widgets/exclusive_sheet.dart';
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

    // The detail host owns the selection sheet's lifecycle; when the selection
    // clears (sheet dismissed), drop the temporary long-press pin so it doesn't
    // linger on the map.
    ref.listen<MapSelection?>(selectedMapEntityProvider, (previous, next) {
      if (next == null && _temporaryPin != null && mounted) {
        setState(() => _temporaryPin = null);
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

    // Active map tool (e.g. route planning) mounted on this single shared map
    // — no second MapController, no pushed screen. The host iterates the
    // registry and never names a concrete tool.
    final activeToolId = ref.watch(activeMapToolProvider);
    final activeTool = activeToolId == null
        ? null
        : ref.watch(mapToolRegistryProvider).get(activeToolId);
    final toolCtx = MapToolContext(ref: ref, mapController: _mapController);
    final toolLayers =
        activeTool != null ? activeTool.buildLayers(toolCtx) : const <Widget>[];
    final toolOverlay = activeTool?.buildOverlay?.call(toolCtx);
    final toolInteraction = activeTool?.interaction?.call(toolCtx);

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

    // Feature layers come from the MapLayerRegistry (composed in app/main.dart)
    // instead of being hand-listed here. Base tiles + attributions sit below,
    // the active tool's layers sit on top.
    final layerCtx =
        MapLayerContext(ref: ref, mapController: _mapController);
    final registryLayers = ref
        .watch(mapLayerRegistryProvider)
        .all
        .expand((d) => d.build(layerCtx));

    final commonMapLayers = <Widget>[
      ...tileLayers,
      ...attributions,
      ...registryLayers,
      // Active tool's layers render on top of everything else.
      ...toolLayers,
    ];

    // Persistent overlays come from the MapOverlayRegistry (composed in
    // app/main.dart): each feature contributes a descriptor and the host lays
    // them out collision-free by slot — no hand-placed Positioned widgets.
    final overlayRegistry = ref.watch(mapOverlayRegistryProvider);
    final overlayCtx = MapOverlayContext(ref: ref);
    final bottomBarDescs = overlayRegistry.inSlot(MapOverlaySlot.bottomBar);
    final overlayWidgets = <Widget>[
      MapOverlayHost(
        bottomBar: bottomBarDescs.isEmpty
            ? null
            : bottomBarDescs.first.build(overlayCtx),
        bottomChildren: [
          for (final d in overlayRegistry.inSlot(MapOverlaySlot.bottomFloating))
            d.build(overlayCtx),
        ],
        topChildren: [
          for (final d in overlayRegistry.inSlot(MapOverlaySlot.topCenter))
            d.build(overlayCtx),
        ],
      ),
      // The active tool's overlay (sheets, hint, close button) on top.
      if (toolOverlay != null) Positioned.fill(child: toolOverlay),
    ];

    // While a tool owns the map, taps feed the tool, interaction can be
    // overridden (e.g. frozen during a waypoint drag), long-press is
    // suppressed and the search bar hidden so the tool's UI is unobstructed.
    final toolActive = activeTool != null;
    void Function(TapPosition, LatLng)? handleTap;
    if (activeTool?.onMapTap != null) {
      handleTap = (tap, point) => activeTool!.onMapTap!(toolCtx, point);
    }
    void Function(PointerDownEvent, LatLng)? handlePointerDown;
    void Function(PointerMoveEvent, LatLng)? handlePointerMove;
    void Function(PointerUpEvent, LatLng)? handlePointerUp;
    if (activeTool?.onPointerDown != null) {
      handlePointerDown =
          (e, ll) => activeTool!.onPointerDown!(toolCtx, e, ll);
    }
    if (activeTool?.onPointerMove != null) {
      handlePointerMove =
          (e, ll) => activeTool!.onPointerMove!(toolCtx, e, ll);
    }
    if (activeTool?.onPointerUp != null) {
      handlePointerUp = (e, ll) => activeTool!.onPointerUp!(toolCtx, e, ll);
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
            onLongPress: (tap, point) {
              if (!toolActive) _handleLongPress(context, point);
            },
            onTap: handleTap,
            onResultSelected: _selectSearchResult,
            onPointerDown: handlePointerDown,
            onPointerMove: handlePointerMove,
            onPointerUp: handlePointerUp,
            interactionOptions: toolInteraction,
            hideSearchBar: toolActive,
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
            onLongPress: (tap, point) {
              if (!toolActive) _handleLongPress(context, point);
            },
            onTap: handleTap,
            onResultSelected: _selectSearchResult,
            onPointerDown: handlePointerDown,
            onPointerMove: handlePointerMove,
            onPointerUp: handlePointerUp,
            interactionOptions: toolInteraction,
            hideSearchBar: toolActive,
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
        // Surface (not the salmon `error`/`primary`, which is near-invisible on
        // the always-light topo) with a white outline so the dropped pin reads.
        child: Stack(
          alignment: Alignment.center,
          children: [
            Icon(Icons.location_pin,
                color: Theme.of(context).colorScheme.onSurface, size: 34),
            Icon(Icons.location_pin,
                color: Theme.of(context).colorScheme.surface, size: 30),
          ],
        ),
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
    _selectCoordinate(point);
  }

  /// Long-press → select the tapped coordinate. The shared detail host renders
  /// the place-info + weather body and the coordinate's own action set through
  /// the same action bar every entity uses. `includeStandardActions: false`
  /// because a bare coordinate's actions (Navigate-as-route, Create marker,
  /// Measure, Plan route) are coordinate-specific and shouldn't mix with the
  /// entity defaults.
  void _selectCoordinate(LatLng point) {
    final isNavigating = ref.read(navigationStateProvider).isActive;
    ref.read(selectedMapEntityProvider.notifier).select(
          MapSelection(
            point: point,
            title: context.l10n.pinSheetSelectedLocation,
            includeStandardActions: false,
            bodyBuilder: (_) => CoordinateDetailBody(point: point),
            extraActions: _coordinateActions(point, isNavigating: isNavigating),
          ),
        );
  }

  /// Search result picked → select it so the shared detail sheet appears,
  /// making search a universal entry point (Navigate / Conditions on any hit).
  /// The map has already panned to the result; here we just surface its actions.
  void _selectSearchResult(LocationSearchResult result) {
    ref.read(selectedMapEntityProvider.notifier).select(
          MapSelection(point: result.position, title: result.title),
        );
  }

  /// The coordinate action set, matching the long-press sheet that preceded the
  /// selection seam: Navigate (route from my location → follow) or Stop, then
  /// Create marker / Create activity / Measure / Plan route.
  List<MapEntityAction> _coordinateActions(LatLng point,
      {required bool isNavigating}) {
    return [
      // Short labels to match the icon-over-label action bar (the registry's
      // standard actions are 'Follow'/'Navigate'/'Save' — same convention).
      isNavigating
          ? MapEntityAction(
              id: 'coord_stop_nav',
              label: 'Stop',
              icon: Icons.stop_circle_outlined,
              isAvailable: (_) => true,
              invoke: (c) {
                c.afterJourneyAction?.call();
                ref.read(navigationStateProvider.notifier).stopNavigation();
              },
            )
          : MapEntityAction(
              id: 'coord_navigate',
              label: 'Navigate',
              icon: Icons.navigation_outlined,
              isAvailable: (_) => true,
              invoke: (c) {
                c.afterJourneyAction?.call();
                _routeFromMyLocation(point);
              },
            ),
      MapEntityAction(
        id: 'coord_marker',
        label: 'Marker',
        icon: Icons.add_location_alt_outlined,
        isAvailable: (_) => true,
        invoke: (c) {
          final prefill = CoordinateDetailBody.resolvedTitle(c.ref, point);
          c.afterJourneyAction?.call();
          _showCreateSheet(context, newLocation: point, prefillName: prefill);
        },
      ),
      MapEntityAction(
        id: 'coord_activity',
        label: 'Activity',
        icon: Icons.outdoor_grill_outlined,
        isAvailable: (_) => true,
        invoke: (c) {
          c.afterJourneyAction?.call();
          _showActivityCreatePicker(context, point);
        },
      ),
      MapEntityAction(
        id: 'coord_measure',
        label: 'Measure',
        icon: Icons.straighten,
        isAvailable: (_) => true,
        invoke: (c) {
          c.afterJourneyAction?.call();
          _navigateToMeasuring(point);
        },
      ),
      MapEntityAction(
        id: 'coord_route',
        label: 'Route',
        icon: Icons.route_outlined,
        isAvailable: (_) => true,
        invoke: (c) {
          c.afterJourneyAction?.call();
          _navigateToRoutePlanning([point]);
        },
      ),
    ];
  }

  /// Open the cross-kind activity picker seeded at the tapped point.
  /// The picker dispatches to the selected kind's create screen via its
  /// descriptor — the map page stays unaware of which kinds exist.
  /// The picker itself renders a sign-in CTA for anonymous users so we
  /// don't need an auth check here.
  void _showActivityCreatePicker(BuildContext context, LatLng point) {
    showExclusiveSheet<void>(
      context,
      builder: (sheetCtx) => activities.ActivityCreatePicker.fromPoint(point),
    );
  }

  /// Start route planning *in place* on this map (no pushed screen / second
  /// map). Seeds the stops, then mounts the route-planning tool — the active
  /// map tool seam handles layers, taps and the planning sheet.
  void _navigateToRoutePlanning(List<LatLng> seeds) {
    final notifier = ref.read(routePlanningProvider.notifier);
    notifier.clear();
    for (final wp in seeds) {
      notifier.addWaypoint(wp);
    }
    ref.read(activeMapToolProvider.notifier).activate(routePlanningToolId);
  }

  /// "Navigate to here" → solve a route from the user's current location to the
  /// tapped point, then go straight into following it. Mirrors the entity
  /// Navigate action exactly: no surprise "route create" planner — if there's
  /// no fix to route from, head straight there as a last resort; if the solve
  /// fails, the same. (Use the explicit "Plan route" action to open the
  /// planner.)
  Future<void> _routeFromMyLocation(LatLng destination) async {
    final notifier = ref.read(activeJourneyProvider.notifier);
    final me = ref.read(locationStateProvider).value;
    if (me == null) {
      notifier.navigateToPoint(destination, label: 'Destination');
      return;
    }
    try {
      final plan =
          await ref.read(routingRepositoryProvider).plan(points: [me, destination]);
      if (!mounted) return;
      notifier.followPath(
            plan.toGeoPath(),
            label: 'Route',
            waypoints: [me, destination],
          );
    } catch (_) {
      if (!mounted) return;
      notifier.navigateToPoint(destination, label: 'Destination');
      AppSnackbars.error(
          context, 'Could not plan a route — heading straight there.');
    }
  }

  /// Measure in place on this map, seeded with the long-pressed point so the
  /// first measurement vertex is already placed (no blank start).
  void _navigateToMeasuring(LatLng startPoint) {
    ref.read(activeMapToolProvider.notifier).activate(measuringToolId);
    ref.read(measuringStateProvider.notifier).addPoint(startPoint);
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
    final result = await showExclusiveSheet<marker_model.Marker>(
      context,
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