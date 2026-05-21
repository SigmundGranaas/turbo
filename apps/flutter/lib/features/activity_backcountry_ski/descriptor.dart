import 'package:flutter/material.dart';
import 'package:flutter_map/flutter_map.dart';

import 'package:turbo/features/activities/api.dart';

import 'widgets/backcountry_ski_create_screen.dart';
import 'widgets/backcountry_ski_detail_sheet.dart';
import 'widgets/backcountry_ski_route_marker.dart';

const _color = Color(0xFF7A3CCB);

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
  buildMapMarker: (summary) => BackcountrySkiRouteMarker(summary: summary),
  buildMapPolyline: (summary) => Polyline(
    points: summary.geometry.coordinates,
    color: _color.withValues(alpha: 0.75),
    strokeWidth: 5.0,
    strokeCap: StrokeCap.round,
    strokeJoin: StrokeJoin.round,
  ),
);
