import 'package:flutter/material.dart';
import 'package:flutter_map/flutter_map.dart';
import 'activity_geometry.dart';
import 'activity_summary.dart';

/// A kind contributes one of these to the shell's
/// [ActivityKindRegistry] at app startup. The shell uses these
/// function pointers to build kind-specific UIs without ever importing a
/// kind feature directly — pure composition, no inheritance.
class ActivityKindDescriptor {
  final String key;
  final String displayName;
  final IconData icon;
  final Color tintColor;
  final Set<ActivityGeometryKind> allowedGeometries;

  /// Build the create screen for this kind, seeded with an initial
  /// geometry (e.g., the map's centre point).
  final Widget Function(BuildContext ctx, ActivityGeometry seedGeometry) buildCreateScreen;

  /// Build the detail screen for an existing activity of this kind.
  final Widget Function(BuildContext ctx, String activityId) buildDetailScreen;

  /// Build the on-map marker for a summary of this kind. Point-geometry
  /// kinds always supply one; LineString / Polygon kinds use it to drop
  /// a start-pin alongside the polyline.
  final Widget Function(ActivitySummary summary)? buildMapMarker;

  /// Build a flutter_map [Polyline] for a route-shaped summary. Returns
  /// null when the kind has no polyline representation (e.g. point
  /// kinds). The shell collects polylines across all kinds and renders
  /// them in one [PolylineLayer] sitting below the marker layer.
  final Polyline Function(ActivitySummary summary)? buildMapPolyline;

  const ActivityKindDescriptor({
    required this.key,
    required this.displayName,
    required this.icon,
    required this.tintColor,
    required this.allowedGeometries,
    required this.buildCreateScreen,
    required this.buildDetailScreen,
    this.buildMapMarker,
    this.buildMapPolyline,
  });
}
