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
  int _seq = 0;

  static const _debounceDelay = Duration(milliseconds: 300);

  @override
  RoutePlanningState build() {
    ref.onDispose(() => _debounce?.cancel());
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

  void clear() {
    _debounce?.cancel();
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
    if (!state.canPlan) {
      // Not enough stops to route — drop any stale plan/spinner.
      _seq++;
      state = state.copyWith(clearPlan: true, clearError: true, isPlanning: false);
      return;
    }
    state = state.copyWith(isPlanning: true, clearError: true);
    _debounce = Timer(_debounceDelay, _replan);
  }

  Future<void> _replan() async {
    final seq = ++_seq;
    final waypoints = state.waypoints;
    final preset = state.presetName;
    try {
      final plan = await ref
          .read(routingRepositoryProvider)
          .plan(points: waypoints, preset: preset);
      if (seq != _seq) return; // superseded by a newer edit
      state = state.copyWith(plan: plan, isPlanning: false, clearError: true);
    } on RoutingException catch (e) {
      if (seq != _seq) return;
      state = state.copyWith(
        isPlanning: false,
        clearPlan: true,
        error: _friendly(e),
      );
    } catch (_) {
      if (seq != _seq) return;
      state = state.copyWith(
        isPlanning: false,
        clearPlan: true,
        error: 'Could not plan a route. Please try again.',
      );
    }
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
