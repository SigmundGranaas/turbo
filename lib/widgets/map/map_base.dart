import 'dart:math';
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
  });

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final colorScheme = Theme.of(context).colorScheme;

    final flutterMap = FlutterMap(
      mapController: mapController,
      options: MapOptions(
        backgroundColor: colorScheme.surfaceBright,
        initialCenter: initialCenter,
        initialZoom: initialZoom,
        maxZoom: 20,
        minZoom: 3,
        onLongPress: onLongPress,
        onTap: onTap,
        onMapReady: onMapReady,
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
      clipBehavior: Clip.none,
      children: [
        Listener(
          onPointerDown: (event) {
            if (onPointerDown != null) {
              final point = event.localPosition;
              final latLng =
              mapController.camera.pointToLatLng(Point(point.dx, point.dy));
              onPointerDown!(event, latLng);
            }
          },
          onPointerMove: (event) {
            if (onPointerMove != null) {
              final point = event.localPosition;
              final latLng =
              mapController.camera.pointToLatLng(Point(point.dx, point.dy));
              onPointerMove!(event, latLng);
            }
          },
          onPointerUp: (event) {
            if (onPointerUp != null) {
              final point = event.localPosition;
              final latLng =
              mapController.camera.pointToLatLng(Point(point.dx, point.dy));
              onPointerUp!(event, latLng);
            }
          },
          child: flutterMap,
        ),
        ...overlayWidgets,
      ],
    );
  }
}