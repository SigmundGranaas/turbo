import 'dart:async';
import 'package:flutter/material.dart';
import 'package:flutter_map/flutter_map.dart';

/// Encapsulates the logic for automatically panning the map when a pointer 
/// is near the screen edges.
class EdgePanHandler {
  final MapController mapController;
  final double threshold;
  final double panSpeed;
  
  Timer? _timer;

  EdgePanHandler({
    required this.mapController,
    this.threshold = 50.0,
    this.panSpeed = 10.0,
  });

  /// Checks the pointer position and starts/stops the auto-pan timer.
  void handlePointerMove(Offset localPosition, Size screenSize) {
    double dx = 0;
    double dy = 0;

    if (localPosition.dx < threshold) dx = -panSpeed;
    if (localPosition.dx > screenSize.width - threshold) dx = panSpeed;
    if (localPosition.dy < threshold) dy = -panSpeed;
    if (localPosition.dy > screenSize.height - threshold) dy = panSpeed;

    if (dx != 0 || dy != 0) {
      _startTimer(dx, dy, screenSize);
    } else {
      stop();
    }
  }

  void _startTimer(double dx, double dy, Size screenSize) {
    if (_timer?.isActive ?? false) return;

    _timer = Timer.periodic(const Duration(milliseconds: 50), (timer) {
      final currentZoom = mapController.camera.zoom;
      
      // Calculate the new map center by offsetting the current screen center
      final newPos = mapController.camera.screenOffsetToLatLng(
        Offset(screenSize.width / 2 + dx, screenSize.height / 2 + dy),
      );
      
      mapController.move(newPos, currentZoom);
    });
  }

  /// Stops any active panning.
  void stop() {
    _timer?.cancel();
    _timer = null;
  }

  void dispose() {
    stop();
  }
}