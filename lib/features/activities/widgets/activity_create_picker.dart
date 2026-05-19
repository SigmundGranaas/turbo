import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:latlong2/latlong.dart';

import '../data/activity_kind_registry.dart';
import '../models/activity_geometry.dart';

/// Bottom-sheet that asks "what kind of activity?" and dispatches to the
/// selected kind's create screen via its descriptor. The shell never
/// imports a specific kind feature.
class ActivityCreatePicker extends ConsumerWidget {
  final LatLng seedLocation;

  const ActivityCreatePicker({super.key, required this.seedLocation});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final registry = ref.watch(activityKindRegistryProvider);
    final kinds = registry.all;
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
              'Pick a kind — each kind has its own form, data, and conditions.',
              style: Theme.of(context).textTheme.bodyMedium,
            ),
            const SizedBox(height: 16),
            if (kinds.isEmpty)
              const Padding(
                padding: EdgeInsets.symmetric(vertical: 24),
                child: Text('No activity kinds are registered in this build.'),
              )
            else
              ...kinds.map((k) => _KindTile(
                    descriptor: k,
                    seedLocation: seedLocation,
                  )),
          ],
        ),
      ),
    );
  }
}

class _KindTile extends StatelessWidget {
  final dynamic descriptor; // ActivityKindDescriptor
  final LatLng seedLocation;
  const _KindTile({required this.descriptor, required this.seedLocation});

  @override
  Widget build(BuildContext context) {
    return ListTile(
      contentPadding: EdgeInsets.zero,
      leading: CircleAvatar(
        backgroundColor: (descriptor.tintColor as Color).withValues(alpha: 0.15),
        foregroundColor: descriptor.tintColor as Color,
        child: Icon(descriptor.icon as IconData),
      ),
      title: Text(descriptor.displayName as String),
      trailing: const Icon(Icons.chevron_right),
      onTap: () {
        Navigator.of(context).pop();
        final seedGeom = ActivityGeometry.fromServer(
          wkt: ActivityGeometry.pointWkt(seedLocation),
          geometryKind: 'POINT',
        );
        Navigator.of(context).push(MaterialPageRoute(
          builder: (ctx) => descriptor.buildCreateScreen(ctx, seedGeom) as Widget,
        ));
      },
    );
  }
}
