import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import 'package:turbo/features/map_view/api.dart';
import 'package:turbo/features/saved_paths/api.dart' show PathStatsPanel;

import '../data/activity_geo_path.dart';
import '../data/activity_summaries_repository.dart';
import '../models/activity_geometry.dart';
import '../models/activity_kind_descriptor.dart';

/// Full-screen container the shell pushes when an activity is tapped (kinds
/// that set [ActivityKindDescriptor.buildDetailContent]). Owns the app bar and
/// a persistent bottom action bar. Route activities now get the same
/// Follow/Conditions/stats treatment as any other path; point activities get
/// Navigate/Conditions — all from the shared entity-action seam.
class ActivityDetailScreen extends ConsumerWidget {
  final ActivityKindDescriptor descriptor;
  final String activityId;
  const ActivityDetailScreen({
    super.key,
    required this.descriptor,
    required this.activityId,
  });

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final summary =
        ref.watch(activitySummariesRepositoryProvider).value?[activityId];
    final geoPath = summary?.geometry.toGeoPath();
    final point = (summary != null &&
            summary.geometry.kind == ActivityGeometryKind.point)
        ? summary.geometry.firstPoint
        : null;

    final body = descriptor.buildDetailContent!(context, activityId);

    final entity = (summary != null && (geoPath != null || point != null))
        ? MapEntityActionContext(
            ref: ref,
            context: context,
            title: summary.name,
            path: geoPath,
            point: point,
            afterJourneyAction: () => Navigator.of(context).pop(),
          )
        : null;

    return Scaffold(
      appBar: AppBar(
        backgroundColor: descriptor.tintColor.withValues(alpha: 0.08),
        title: Text(descriptor.displayName),
        elevation: 0,
      ),
      body: SafeArea(
        bottom: false,
        child: SingleChildScrollView(
          padding: const EdgeInsets.only(bottom: 96),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              if (geoPath != null)
                Padding(
                  padding: const EdgeInsets.fromLTRB(16, 16, 16, 0),
                  child: PathStatsPanel(path: geoPath),
                ),
              body,
            ],
          ),
        ),
      ),
      bottomNavigationBar: entity == null
          ? null
          : SafeArea(
              child: Padding(
                padding: const EdgeInsets.fromLTRB(8, 0, 8, 8),
                child: MapEntityActionBar(entity: entity),
              ),
            ),
    );
  }
}
