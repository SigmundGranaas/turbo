import 'package:flutter/material.dart';

import 'package:turbo/features/activities/api.dart';

import 'widgets/packrafting_create_screen.dart';
import 'widgets/packrafting_detail_sheet.dart';
import 'widgets/packrafting_route_marker.dart';

final packraftingActivityKindDescriptor = ActivityKindDescriptor(
  key: 'packrafting',
  displayName: 'Packrafting',
  icon: Icons.kayaking_outlined,
  tintColor: const Color(0xFFEF6C00),
  allowedGeometries: {ActivityGeometryKind.lineString},
  buildCreateScreen: (ctx, seed) => PackraftingCreateScreen(seedGeometry: seed),
  buildDetailScreen: (ctx, id) => PackraftingDetailSheet(activityId: id),
  buildMapMarker: (summary) => PackraftingRouteMarker(summary: summary),
);
