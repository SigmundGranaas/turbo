import 'package:flutter/material.dart';
import 'package:flutter_map/flutter_map.dart';

import 'package:turbo/features/activities/api.dart';

import 'models/hiking_analysis_extras.dart';
import 'widgets/hiking_create_screen.dart';
import 'widgets/hiking_detail_sheet.dart';

const _color = Color(0xFF2E7D32);

final hikingActivityKindDescriptor = ActivityKindDescriptor(
  key: 'hiking',
  displayName: 'Hiking',
  icon: Icons.hiking_outlined,
  tintColor: _color,
  allowedGeometries: {ActivityGeometryKind.lineString},
  buildCreateScreen: (ctx, seed) => HikingCreateScreen(seedGeometry: seed),
  buildDetailScreen: (ctx, id) => HikingDetailSheet(activityId: id),
  buildDetailContent: (ctx, id) => HikingDetailSheet(activityId: id),
  parseAnalysisExtras: (slices) => HikingAnalysisExtras.tryParse(slices['hiking']),
  buildMapPolyline: (summary) => Polyline(
    points: summary.geometry.coordinates,
    color: _color.withValues(alpha: 0.75),
    strokeWidth: 4.5,
    strokeCap: StrokeCap.round,
    strokeJoin: StrokeJoin.round,
  ),
);
