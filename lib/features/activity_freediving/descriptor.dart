import 'package:flutter/material.dart';

import 'package:turbo/features/activities/api.dart';

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
  buildMapMarker: (summary) => FreedivingMapMarker(summary: summary),
);
