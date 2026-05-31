import 'package:flutter/material.dart';

import 'package:turbo/features/activities/api.dart';

import 'models/freediving_analysis_extras.dart';
import 'widgets/freediving_create_screen.dart';
import 'widgets/freediving_detail_sheet.dart';
import 'widgets/freediving_map_marker.dart';

final freedivingActivityKindDescriptor = ActivityKindDescriptor(
  key: 'freediving',
  displayName: 'Freediving',
  icon: Icons.scuba_diving_outlined,
  tintColor: const Color(0xFF1565C0),
  allowedGeometries: {ActivityGeometryKind.point},
  buildCreateScreen: (ctx, seed) => FreedivingCreateScreen(seedGeometry: seed),
  buildDetailScreen: (ctx, id) => FreedivingDetailSheet(activityId: id),
  // Opt into the full-screen shell. The viz forecast row + analysis
  // surface together no longer fit in a 60% bottom sheet.
  buildDetailContent: (ctx, id) => FreedivingDetailSheet(activityId: id),
  parseAnalysisExtras: (slices) => FreedivingAnalysisExtras.tryParse(slices['freediving']),
  buildMapMarker: (summary) => FreedivingMapMarker(summary: summary),
);
