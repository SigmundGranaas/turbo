import 'package:flutter/material.dart';
import 'package:flutter_map/flutter_map.dart';

import 'package:turbo/features/activities/api.dart';

import 'models/backcountry_ski_analysis_extras.dart';
import 'widgets/backcountry_ski_create_screen.dart';
import 'widgets/backcountry_ski_detail_sheet.dart';
import 'widgets/backcountry_ski_route_marker.dart';

const _color = Color(0xFF5E72A5);

/// Single descriptor the backcountry ski kind contributes to the shell's
/// registry. The shell never imports this feature directly — only the
/// list-of-descriptors at app bootstrap does.
final backcountrySkiActivityKindDescriptor = ActivityKindDescriptor(
  key: 'backcountry_ski',
  displayName: 'Backcountry skiing',
  icon: Icons.downhill_skiing_outlined,
  tintColor: _color,
  allowedGeometries: {ActivityGeometryKind.lineString},
  buildCreateScreen: (ctx, seed) =>
      BackcountrySkiCreateScreen(seedGeometry: seed),
  buildDetailScreen: (ctx, id) => BackcountrySkiDetailSheet(activityId: id),
  // Opt into the full-screen shell — analyses with multiple warnings,
  // per-aspect loading row, and driver cards deserve the whole
  // viewport, not 60% of it.
  buildDetailContent: (ctx, id) => BackcountrySkiDetailSheet(activityId: id),
  parseAnalysisExtras: (slices) =>
      BackcountrySkiAnalysisExtras.tryParse(slices['backcountry_ski']),
  buildMapMarker: (summary) => BackcountrySkiRouteMarker(summary: summary),
  buildMapPolyline: (summary) => Polyline(
    points: summary.geometry.coordinates,
    color: _color.withValues(alpha: 0.75),
    strokeWidth: 5.0,
    strokeCap: StrokeCap.round,
    strokeJoin: StrokeJoin.round,
  ),
);
