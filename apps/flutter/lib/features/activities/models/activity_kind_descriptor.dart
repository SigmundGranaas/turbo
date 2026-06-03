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
  /// Legacy seam: rendered inside a modal bottom sheet by the map layer
  /// when [buildDetailContent] is null.
  final Widget Function(BuildContext ctx, String activityId) buildDetailScreen;

  /// Full-screen detail-content builder. Returns a scrollable [Widget]
  /// (NOT a [Scaffold]) — the shell's [ActivityDetailScreen] owns the
  /// chrome (app bar, action bar, hero collapse). When set, the map
  /// layer pushes a full-screen route into [ActivityDetailScreen]
  /// instead of opening a bottom sheet. Optional for backwards-compat
  /// while kinds migrate.
  final Widget Function(BuildContext ctx, String activityId)? buildDetailContent;

  /// Build a flutter_map [Polyline] for a route-shaped summary. Returns
  /// null when the kind has no polyline representation (e.g. point
  /// kinds). The shell collects polylines across all kinds and renders
  /// them in one [PolylineLayer] sitting below the marker layer.
  final Polyline Function(ActivitySummary summary)? buildMapPolyline;

  /// Parse this kind's slice out of `analysis.kindSlices`. The result
  /// is whatever typed `*AnalysisExtras` object the per-kind feature
  /// defined (or `null` if the slice is missing/malformed). Returning
  /// `Object?` instead of a generic keeps the descriptor heterogeneous
  /// (different kinds, different extras shapes) — callers cast back
  /// to the concrete type they own. The benefit: the magic string
  /// `kindSlices['xc_ski']` lives in one place keyed by [key],
  /// so backend renames break compile-time references in the kind's
  /// own descriptor instead of silently emptying the UI.
  final Object? Function(Map<String, Object?> kindSlices)? parseAnalysisExtras;

  const ActivityKindDescriptor({
    required this.key,
    required this.displayName,
    required this.icon,
    required this.tintColor,
    required this.allowedGeometries,
    required this.buildCreateScreen,
    required this.buildDetailScreen,
    this.buildDetailContent,
    this.buildMapPolyline,
    this.parseAnalysisExtras,
  });
}
