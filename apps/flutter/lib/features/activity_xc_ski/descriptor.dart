import 'package:flutter/material.dart';
import 'package:flutter_map/flutter_map.dart';

import 'package:turbo/features/activities/api.dart';

import 'models/xc_ski_analysis_extras.dart';
import 'widgets/xc_ski_create_screen.dart';
import 'widgets/xc_ski_detail_sheet.dart';
import 'widgets/xc_ski_route_marker.dart';

const _color = Color(0xFF0288D1);

final xcSkiActivityKindDescriptor = ActivityKindDescriptor(
  key: 'xc_ski',
  displayName: 'XC skiing',
  icon: Icons.snowshoeing_outlined,
  tintColor: _color,
  allowedGeometries: {ActivityGeometryKind.lineString},
  buildCreateScreen: (ctx, seed) => XcSkiCreateScreen(seedGeometry: seed),
  buildDetailScreen: (ctx, id) => XcSkiDetailSheet(activityId: id),
  // Opt into the full-screen shell — same content, just hosted in
  // ActivityDetailScreen with the shell's app bar and chrome instead
  // of a 60% bottom sheet.
  buildDetailContent: (ctx, id) => XcSkiDetailSheet(activityId: id),
  buildMapMarker: (summary) => XcSkiRouteMarker(summary: summary),
  parseAnalysisExtras: (slices) => XcSkiAnalysisExtras.tryParse(slices['xc_ski']),
  buildMapPolyline: (summary) => Polyline(
    points: summary.geometry.coordinates,
    color: _color.withValues(alpha: 0.75),
    strokeWidth: 4.0,
    strokeCap: StrokeCap.round,
    strokeJoin: StrokeJoin.round,
    // Dashed stroke hints at the "groomed track" visual for xc skiing.
    pattern: StrokePattern.dashed(segments: const [10.0, 6.0]),
  ),
);
