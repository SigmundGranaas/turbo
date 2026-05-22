import 'package:flutter/material.dart';
import 'package:latlong2/latlong.dart';
import '../data/measuring_state_notifier.dart';

/// Handles the complex gesture logic for freehand drawing, including
/// multi-finger pan detection and drawing commitment thresholds.
class FreehandGestureHandler {
  final MeasuringStateNotifier notifier;
  final double Function() getSensitivity;
  final VoidCallback onEdgePanStop;
  final Function(Offset) onMove;

  final Set<int> _activePointers = {};
  bool _isDrawingValid = false;
  Offset? _lastPointerScreenPos;
  LatLng? _pendingStartPoint;
  
  // Threshold to distinguish between a tap/pan start and a drawing stroke
  static const double _commitmentThreshold = 5.0;

  FreehandGestureHandler({
    required this.notifier,
    required this.getSensitivity,
    required this.onEdgePanStop,
    required this.onMove,
  });

  void handlePointerDown(PointerDownEvent event, LatLng point) {
    _activePointers.add(event.pointer);
    
    if (_activePointers.length == 1) {
      // Buffer the point until we see enough movement to commit to a stroke.
      _pendingStartPoint = point;
      _lastPointerScreenPos = event.localPosition;
      _isDrawingValid = true;
    } else {
      // Multi-finger detected: Canceled any pending drawing to allow map pan.
      _cancelDrawing();
    }
  }

  void handlePointerMove(PointerMoveEvent event, LatLng point) {
    if (!_isDrawingValid || _activePointers.length != 1 || _lastPointerScreenPos == null) {
      return;
    }

    onMove(event.localPosition);

    final distance = (event.localPosition - _lastPointerScreenPos!).distance;

    // 1. Check for Commitment: If we moved past the threshold, start the stroke.
    if (_pendingStartPoint != null) {
      if (distance > _commitmentThreshold) {
        notifier.addPoint(_pendingStartPoint!);
        _pendingStartPoint = null;
      } else {
        return; // Haven't moved enough to commit yet.
      }
    }

    // 2. Add points based on sensitivity.
    final sensitivity = getSensitivity();
    if (distance > sensitivity) {
      notifier.addPoint(point);
      _lastPointerScreenPos = event.localPosition;
    }
  }

  void handlePointerUp(PointerUpEvent event, LatLng point) {
    _activePointers.remove(event.pointer);
    
    // If we just tapped (released without moving enough to commit a stroke),
    // add the single point now as a regular tap.
    if (_pendingStartPoint != null && _activePointers.isEmpty) {
      notifier.addPoint(_pendingStartPoint!);
    }

    if (_activePointers.isEmpty) {
      _cancelDrawing();
    }
  }

  void _cancelDrawing() {
    _isDrawingValid = false;
    _lastPointerScreenPos = null;
    _pendingStartPoint = null;
    onEdgePanStop();
  }

  void reset() {
    _activePointers.clear();
    _cancelDrawing();
  }
}