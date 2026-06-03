import 'package:flutter/material.dart';
import 'package:flutter_map/flutter_map.dart';

import 'package:turbo/features/activities/api.dart';

import 'models/packrafting_analysis_extras.dart';
import 'widgets/packrafting_create_screen.dart';
import 'widgets/packrafting_detail_sheet.dart';

const _color = Color(0xFF00838F);

final packraftingActivityKindDescriptor = ActivityKindDescriptor(
  key: 'packrafting',
  displayName: 'Packrafting',
  icon: Icons.kayaking_outlined,
  tintColor: _color,
  allowedGeometries: {ActivityGeometryKind.lineString},
  buildCreateScreen: (ctx, seed) => PackraftingCreateScreen(seedGeometry: seed),
  buildDetailScreen: (ctx, id) => PackraftingDetailSheet(activityId: id),
  buildDetailContent: (ctx, id) => PackraftingDetailSheet(activityId: id),
  parseAnalysisExtras: (slices) =>
      PackraftingAnalysisExtras.tryParse(slices['packrafting']),
  buildMapPolyline: (summary) => Polyline(
    points: summary.geometry.coordinates,
    color: _color.withValues(alpha: 0.85),
    strokeWidth: 4.5,
    strokeCap: StrokeCap.round,
    strokeJoin: StrokeJoin.round,
  ),
);
