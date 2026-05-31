import 'dart:async';

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:latlong2/latlong.dart';

import '../providers/routing_providers.dart';
import 'routing_api_client.dart';
import 'route_planning_state.dart';

/// Owns the route-planning screen's state and (re)solves the route as the
/// user edits waypoints or switches preset. Solves are debounced so rapid
/// edits collapse into one request, and stale responses are dropped via a
/// monotonic sequence guard.
class RoutePlanningNotifier extends Notifier<RoutePlanningState> {
  Timer? _debounce;
  StreamSubscription<RouteStreamEvent>? _sub;
  int _seq = 0;

  static const _debounceDelay = Duration(milliseconds: 300);

  @override
  RoutePlanningState build() {
    ref.onDispose(() {
      _debounce?.cancel();
      _sub?.cancel();
    });
    return const RoutePlanningState();
  }

  void addWaypoint(LatLng point) {
    state = state.copyWith(waypoints: [...state.waypoints, point]);
    _scheduleReplan();
  }

  void undoLast() {
    if (state.waypoints.isEmpty) return;
    final next = state.waypoints.sublist(0, state.waypoints.length - 1);
    state = state.copyWith(waypoints: next);
    _scheduleReplan();
  }

  void removeAt(int index) {
    if (index < 0 || index >= state.waypoints.length) return;
    final next = [...state.waypoints]..removeAt(index);
    state = state.copyWith(waypoints: next);
    _scheduleReplan();
  }

  /// Reposition a stop (live drag). Updates the marker immediately; the
  /// re-solve is debounced so a continuous drag fires one request on
  /// settle, not one per frame.
  void moveWaypoint(int index, LatLng point) {
    if (index < 0 || index >= state.waypoints.length) return;
    final next = [...state.waypoints]..[index] = point;
    state = state.copyWith(waypoints: next);
    _scheduleReplan();
  }

  void clear() {
    _debounce?.cancel();
    _sub?.cancel();
    _seq++; // invalidate any in-flight solve
    state = const RoutePlanningState();
  }

  void setPreset(String name) {
    if (name == state.presetName) return;
    state = state.copyWith(presetName: name);
    _scheduleReplan();
  }

  void _scheduleReplan() {
    _debounce?.cancel();
    _sub?.cancel();
    if (!state.canPlan) {
      // Not enough stops to route — drop any stale plan/preview/spinner.
      _seq++;
      state = state.copyWith(
        clearPlan: true,
        clearPreview: true,
        clearError: true,
        isPlanning: false,
      );
      return;
    }
    state = state.copyWith(isPlanning: true, clearError: true);
    _debounce = Timer(_debounceDelay, _replan);
  }

  void _replan() {
    final seq = ++_seq;
    final waypoints = state.waypoints;
    final preset = state.presetName;
    final stream = ref
        .read(routingRepositoryProvider)
        .planStream(points: waypoints, preset: preset);
    _sub = stream.listen(
      (event) {
        if (seq != _seq) return;
        switch (event) {
          case RouteProgress(:final geometry):
            state = state.copyWith(previewGeometry: geometry);
          case RouteResult(:final plan):
            state = state.copyWith(
              plan: plan,
              isPlanning: false,
              clearPreview: true,
              clearError: true,
            );
        }
      },
      onError: (Object e) {
        if (seq != _seq) return;
        state = state.copyWith(
          isPlanning: false,
          clearPlan: true,
          clearPreview: true,
          error: e is RoutingException ? _friendly(e) : 'Could not plan a route. Please try again.',
        );
      },
      onDone: () {
        if (seq != _seq) return;
        // Stream ended without a result frame (shouldn't normally happen).
        if (state.isPlanning) state = state.copyWith(isPlanning: false);
      },
    );
  }

  String _friendly(RoutingException e) => switch (e.kind) {
        RoutingErrorKind.noRoute =>
          'No route found between these stops — try moving them off water or steep terrain.',
        RoutingErrorKind.badRequest => 'That route request was not valid.',
        RoutingErrorKind.network =>
          'Could not reach the routing service. Check your connection.',
        RoutingErrorKind.server => 'The routing service had a problem. Try again.',
      };
}

final routePlanningProvider =
    NotifierProvider.autoDispose<RoutePlanningNotifier, RoutePlanningState>(
  RoutePlanningNotifier.new,
);
