import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:latlong2/latlong.dart';

import '../data/activity_kind_registry.dart';
import '../models/activity_geometry.dart';
import '../models/activity_kind_descriptor.dart';

/// Bottom-sheet that asks "what kind of activity?" and dispatches to the
/// selected kind's create screen via its descriptor. The shell never
/// imports a specific kind feature.
///
/// Filters the candidate kinds by the seed geometry's shape: pinning a
/// Point only offers point-based kinds; promoting a recorded track
/// (LineString) only offers line-based kinds. This means a long-press
/// in the map → "Add activity here" and a saved-path "Save as activity"
/// share one widget but never confuse the user with mismatched kinds.
class ActivityCreatePicker extends ConsumerWidget {
  final ActivityGeometry seedGeometry;

  const ActivityCreatePicker({super.key, required this.seedGeometry});

  /// Convenience constructor for the long-press flow, where the seed
  /// is just a point.
  factory ActivityCreatePicker.fromPoint(LatLng point, {Key? key}) =>
      ActivityCreatePicker(
        key: key,
        seedGeometry: ActivityGeometry.fromServer(
          wkt: ActivityGeometry.pointWkt(point),
          geometryKind: 'POINT',
        ),
      );

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final registry = ref.watch(activityKindRegistryProvider);
    final kinds = registry.all
        .where((k) => k.allowedGeometries.contains(seedGeometry.kind))
        .toList();
    return SafeArea(
      child: Padding(
        padding: const EdgeInsets.fromLTRB(16, 16, 16, 24),
        child: Column(
          mainAxisSize: MainAxisSize.min,
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Text(
              'New activity',
              style: Theme.of(context).textTheme.headlineSmall,
            ),
            const SizedBox(height: 8),
            Text(
              _subtitleFor(seedGeometry.kind),
              style: Theme.of(context).textTheme.bodyMedium,
            ),
            const SizedBox(height: 16),
            if (kinds.isEmpty)
              Padding(
                padding: const EdgeInsets.symmetric(vertical: 24),
                child: Text(_emptyMessageFor(seedGeometry.kind)),
              )
            else
              ...kinds.map((k) => _KindTile(
                    descriptor: k,
                    seedGeometry: seedGeometry,
                  )),
          ],
        ),
      ),
    );
  }

  static String _subtitleFor(ActivityGeometryKind kind) => switch (kind) {
        ActivityGeometryKind.point =>
          'Pick a kind — each kind has its own form, data, and conditions.',
        ActivityGeometryKind.lineString =>
          'Pick a kind to record this route as.',
        ActivityGeometryKind.polygon =>
          'Pick a kind to record this area as.',
      };

  static String _emptyMessageFor(ActivityGeometryKind kind) => switch (kind) {
        ActivityGeometryKind.point =>
          'No point-based activity kinds are registered in this build.',
        ActivityGeometryKind.lineString =>
          'No route-based activity kinds are registered in this build.',
        ActivityGeometryKind.polygon =>
          'No area-based activity kinds are registered in this build.',
      };
}

class _KindTile extends StatelessWidget {
  final ActivityKindDescriptor descriptor;
  final ActivityGeometry seedGeometry;
  const _KindTile({required this.descriptor, required this.seedGeometry});

  @override
  Widget build(BuildContext context) {
    return ListTile(
      contentPadding: EdgeInsets.zero,
      leading: CircleAvatar(
        backgroundColor: descriptor.tintColor.withValues(alpha: 0.15),
        foregroundColor: descriptor.tintColor,
        child: Icon(descriptor.icon),
      ),
      title: Text(descriptor.displayName),
      trailing: const Icon(Icons.chevron_right),
      onTap: () {
        Navigator.of(context).pop();
        Navigator.of(context).push(MaterialPageRoute(
          builder: (ctx) => descriptor.buildCreateScreen(ctx, seedGeometry),
        ));
      },
    );
  }
}
