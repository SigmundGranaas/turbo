import 'package:flutter/material.dart';

import 'package:turbo/features/activities/api.dart';

import 'widgets/hiking_create_screen.dart';
import 'widgets/hiking_detail_sheet.dart';
import 'widgets/hiking_route_marker.dart';

final hikingActivityKindDescriptor = ActivityKindDescriptor(
  key: 'hiking',
  displayName: 'Hiking',
  icon: Icons.hiking_outlined,
  tintColor: const Color(0xFF2E7D32),
  allowedGeometries: {ActivityGeometryKind.lineString},
  buildCreateScreen: (ctx, seed) => HikingCreateScreen(seedGeometry: seed),
  buildDetailScreen: (ctx, id) => HikingDetailSheet(activityId: id),
  buildMapMarker: (summary) => HikingRouteMarker(summary: summary),
);
