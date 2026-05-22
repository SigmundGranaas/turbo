import 'dart:async';

import 'package:flutter/animation.dart';
import 'package:flutter_map/flutter_map.dart';
import 'package:latlong2/latlong.dart';

void zoomIn(MapController controller, TickerProvider ticker) {
  animatedMapMove(
      controller.camera.center,
      controller.camera.zoom + 1,
      controller,
      ticker
  );
}

void zoomOut(MapController controller, TickerProvider ticker) {
  animatedMapMove(
      controller.camera.center,
      controller.camera.zoom - 1,
      controller,
      ticker
  );
}

// The function now returns a Future<void> that completes when the animation finishes.
Future<void> animatedMapMove(LatLng destLocation, double destZoom, MapController mapController, TickerProvider provider) {
  final completer = Completer<void>();

  final latTween = Tween<double>(
      begin: mapController.camera.center.latitude,
      end: destLocation.latitude);
  final lngTween = Tween<double>(
      begin: mapController.camera.center.longitude,
      end: destLocation.longitude);

  final zoomTween = Tween<double>(begin: mapController.camera.zoom, end: destZoom);

  final controller = AnimationController(
      duration: const Duration(milliseconds: 500), vsync: provider);

  Animation<double> animation = CurvedAnimation(parent: controller, curve: Curves.fastOutSlowIn);

  controller.addListener(() {
    mapController.move(
        LatLng(latTween.evaluate(animation), lngTween.evaluate(animation)),
        zoomTween.evaluate(animation));
  });

  animation.addStatusListener((status) {
    if (status == AnimationStatus.completed) {
      controller.dispose();
      if (!completer.isCompleted) completer.complete();
    } else if (status == AnimationStatus.dismissed) {
      controller.dispose();
      if (!completer.isCompleted) completer.complete(); // Also complete on dismiss
    }
  });

  controller.forward();
  return completer.future;
}