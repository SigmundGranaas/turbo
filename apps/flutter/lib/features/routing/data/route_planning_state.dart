import 'package:latlong2/latlong.dart';

import '../models/route_models.dart';

/// UI state for the interactive route-planning screen: the ordered
/// waypoints the user has dropped, the chosen preset, and the latest
/// solve (or in-flight / error status).
class RoutePlanningState {
  /// Ordered stops (start, vias, end) in tap order.
  final List<LatLng> waypoints;

  /// Selected preset name (see [RoutePreset.name]); defaults to "balanced".
  final String presetName;

  /// The most recent successful solve, or null before the first solve /
  /// after a clear.
  final RoutePlan? plan;

  /// True while a solve is in flight (covers the debounce + request).
  final bool isPlanning;

  /// User-facing error from the last solve, or null.
  final String? error;

  const RoutePlanningState({
    this.waypoints = const [],
    this.presetName = 'balanced',
    this.plan,
    this.isPlanning = false,
    this.error,
  });

  bool get canPlan => waypoints.length >= 2;
  bool get isEmpty => waypoints.isEmpty;

  RoutePlanningState copyWith({
    List<LatLng>? waypoints,
    String? presetName,
    RoutePlan? plan,
    bool clearPlan = false,
    bool? isPlanning,
    String? error,
    bool clearError = false,
  }) {
    return RoutePlanningState(
      waypoints: waypoints ?? this.waypoints,
      presetName: presetName ?? this.presetName,
      plan: clearPlan ? null : (plan ?? this.plan),
      isPlanning: isPlanning ?? this.isPlanning,
      error: clearError ? null : (error ?? this.error),
    );
  }
}
