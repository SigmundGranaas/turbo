import 'package:flutter/material.dart';
import 'package:flutter_map/flutter_map.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:latlong2/latlong.dart';

class MapBase extends ConsumerWidget {
  final MapController mapController;
  final List<Widget> mapLayers;
  final List<Widget> overlayWidgets;
  final Function(TapPosition, LatLng)? onLongPress;
  final Function(TapPosition, LatLng)? onTap;

  const MapBase({
    super.key,
    required this.mapController,
    required this.mapLayers,
    required this.overlayWidgets,
    this.onLongPress,
    this.onTap,
  });

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    return Stack(
      children: [
        FlutterMap(
          mapController: mapController,
          options: MapOptions(
            initialCenter: const LatLng(65.0, 13.0),
            initialZoom: 5,
            maxZoom: 20,
            minZoom: 3,
            onLongPress: onLongPress,
            onTap: onTap,
            interactionOptions: const InteractionOptions(
              flags: InteractiveFlag.all,
              enableMultiFingerGestureRace: true,
              pinchZoomThreshold: 0.2,
              pinchMoveThreshold: 40,
              rotationThreshold: 5.0,
            ),
          ),
          children: mapLayers,
        ),
        ...overlayWidgets,
      ],
    );
  }
}