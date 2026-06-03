import 'package:flutter/material.dart';

import 'package:turbo/features/activities/api.dart';

import 'models/fishing_analysis_extras.dart';
import 'widgets/fishing_create_screen.dart';
import 'widgets/fishing_detail_sheet.dart';

/// Single descriptor the fishing kind contributes to the shell's
/// [ActivityKindRegistry]. Pure composition: the shell's create-picker,
/// detail router, and map layer use these function pointers without ever
/// importing the kind feature directly.
final fishingActivityKindDescriptor = ActivityKindDescriptor(
  key: 'fishing',
  displayName: 'Fishing',
  icon: Icons.set_meal_outlined,
  tintColor: const Color(0xFF1E6FB8),
  allowedGeometries: {ActivityGeometryKind.point},
  buildCreateScreen: (ctx, seed) => FishingCreateScreen(seedGeometry: seed),
  buildDetailScreen: (ctx, id) => FishingDetailSheet(activityId: id),
  buildDetailContent: (ctx, id) => FishingDetailSheet(activityId: id),
  parseAnalysisExtras: (slices) => FishingAnalysisExtras.tryParse(slices['fishing']),
);
