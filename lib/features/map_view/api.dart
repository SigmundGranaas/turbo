/// The public API for the Map View feature.
library;

import 'dart:async';
import 'package:flutter/animation.dart';
import 'package:flutter_map/flutter_map.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:latlong2/latlong.dart';
import 'package:turbo/features/map_view/data/map_view_state_notifier.dart';

import 'models/map_view_state.dart';

// 1. Export the main UI entry point for this feature.
export 'widgets/main_map_page.dart' show MainMapPage;

// 2. Export the public state model.
export 'models/map_view_state.dart' show MapViewState;

// 3. Export the public state provider.
final mapViewStateProvider =
NotifierProvider<MapViewStateNotifier, MapViewState>(
  MapViewStateNotifier.new,
);

/// 4. (Recommended) Provide a clean, mockable API wrapper class.
/// This decouples consumers from Riverpod's syntax and the MapController.
final mapApiProvider = Provider<MapApi>((ref) => MapApi());

class MapApi {


  /// Animates the map to a new destination.
  ///
  /// This should be the primary way other features interact with the map's position.
  Future<void> animatedMapMove(
      LatLng destLocation,
      double destZoom, {
        required TickerProvider vsync,
        MapController? mapController,
      }) {
    // This is a simplified version. A more robust implementation would use a
    // global key or a different mechanism to get the current MapController
    // without passing it directly. For now, we assume it's available.
    if (mapController == null) return Future.value();

    final completer = Completer<void>();
    final latTween = Tween<double>(
        begin: mapController.camera.center.latitude, end: destLocation.latitude);
    final lngTween = Tween<double>(
        begin: mapController.camera.center.longitude,
        end: destLocation.longitude);
    final zoomTween =
    Tween<double>(begin: mapController.camera.zoom, end: destZoom);

    final controller = AnimationController(
        duration: const Duration(milliseconds: 500), vsync: vsync);
    final animation =
    CurvedAnimation(parent: controller, curve: Curves.fastOutSlowIn);

    controller.addListener(() {
      mapController.move(
          LatLng(latTween.evaluate(animation), lngTween.evaluate(animation)),
          zoomTween.evaluate(animation));
    });

    animation.addStatusListener((status) {
      if (status == AnimationStatus.completed ||
          status == AnimationStatus.dismissed) {
        controller.dispose();
        if (!completer.isCompleted) completer.complete();
      }
    });

    controller.forward();
    return completer.future;
  }
}