import 'package:flutter/material.dart';
import 'package:flutter_map/flutter_map.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:latlong2/latlong.dart';

class MapBase extends ConsumerWidget {
  final MapController mapController;
  final List<Widget> mapLayers;
  final List<Widget> overlayWidgets;
  final double initialZoom;
  final LatLng initialCenter;
  final Function(TapPosition, LatLng)? onLongPress;
  final Function(TapPosition, LatLng)? onTap;
  final Function()? onMapReady;
  final void Function(PointerDownEvent, LatLng)? onPointerDown;
  final void Function(PointerMoveEvent, LatLng)? onPointerMove;
  final void Function(PointerUpEvent, LatLng)? onPointerUp;
  final InteractionOptions? interactionOptions;
  final MapEventCallback? onMapEvent; // Changed from onPositionChanged

  /// The coordinate reference system the map renders in. Defaults to Web
  /// Mercator; switches to UTM33 (EPSG:25833) for high-detail Norwegian topo.
  final Crs crs;

  const MapBase({
    super.key,
    required this.mapController,
    required this.mapLayers,
    required this.overlayWidgets,
    this.initialZoom = 5,
    this.initialCenter = const LatLng(65.0, 13.0),
    this.onLongPress,
    this.onTap,
    this.onMapReady,
    this.onPointerDown,
    this.onPointerMove,
    this.onPointerUp,
    this.interactionOptions,
    this.onMapEvent, // Changed
    this.crs = const Epsg3857(),
  });

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final colorScheme = Theme.of(context).colorScheme;

    final flutterMap = FlutterMap(
      // Rebuild the map from scratch when the CRS changes (e.g. toggling the
      // high-detail UTM33 topo base). flutter_map reads `crs` once at creation,
      // so a fresh element is required for the switch to take effect and to
      // drop tiles cached against the previous projection.
      key: ValueKey('flutter_map_${crs.code}'),
      mapController: mapController,
      options: MapOptions(
        crs: crs,
        backgroundColor: colorScheme.surfaceBright,
        initialCenter: initialCenter,
        initialZoom: initialZoom,
        maxZoom: 20,
        minZoom: 3,
        onLongPress: onLongPress,
        onTap: onTap,
        onMapReady: onMapReady,
        onMapEvent: onMapEvent, // Use the correct callback
        interactionOptions: interactionOptions ??
            const InteractionOptions(
              flags: InteractiveFlag.all,
              enableMultiFingerGestureRace: true,
              pinchZoomThreshold: 0.2,
              pinchMoveThreshold: 40,
              rotationThreshold: 5.0,
            ),
      ),
      children: mapLayers,
    );

    return Stack(
      children: [
        Positioned.fill(
          child: Listener(
            onPointerDown: (event) {
              if (onPointerDown != null) {
                final point = event.localPosition;
                final latLng =
                mapController.camera.screenOffsetToLatLng(point);
                onPointerDown!(event, latLng);
              }
            },
            onPointerMove: (event) {
              if (onPointerMove != null) {
                final point = event.localPosition;
                final latLng =
                mapController.camera.screenOffsetToLatLng(point);
                onPointerMove!(event, latLng);
              }
            },
            onPointerUp: (event) {
              if (onPointerUp != null) {
                final point = event.localPosition;
                final latLng =
                mapController.camera.screenOffsetToLatLng(point);
                onPointerUp!(event, latLng);
              }
            },
            child: flutterMap,
          ),
        ),
        Positioned.fill(
          child: SafeArea(
            left: false, // Map usually looks better if side-to-side padding is managed by widgets
            right: false,
            child: Stack(
              children: overlayWidgets,
            ),
          ),
        ),
      ],
    );
  }
}