import 'package:flutter/material.dart';

import 'package:turbo/features/activities/api.dart';

import 'widgets/xc_ski_create_screen.dart';
import 'widgets/xc_ski_detail_sheet.dart';
import 'widgets/xc_ski_route_marker.dart';

final xcSkiActivityKindDescriptor = ActivityKindDescriptor(
  key: 'xc_ski',
  displayName: 'XC skiing',
  icon: Icons.snowshoeing_outlined,
  tintColor: const Color(0xFF00838F),
  allowedGeometries: {ActivityGeometryKind.lineString},
  buildCreateScreen: (ctx, seed) => XcSkiCreateScreen(seedGeometry: seed),
  buildDetailScreen: (ctx, id) => XcSkiDetailSheet(activityId: id),
  buildMapMarker: (summary) => XcSkiRouteMarker(summary: summary),
);
