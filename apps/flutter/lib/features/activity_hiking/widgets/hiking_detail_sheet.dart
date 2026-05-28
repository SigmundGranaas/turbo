import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import 'package:turbo/core/widgets/sheet_action_bar.dart';
import 'package:turbo/features/activities/api.dart' show ActivityGeometry;

import '../data/hiking_repository.dart';
import '../models/hiking_activity.dart';
import 'hiking_conditions_panel.dart';
import 'hiking_create_screen.dart';

class HikingDetailSheet extends ConsumerWidget {
  final String activityId;
  const HikingDetailSheet({super.key, required this.activityId});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final async = ref.watch(hikingActivityProvider(activityId));
    return SafeArea(child: Padding(
      padding: const EdgeInsets.all(16),
      child: async.when(
        loading: () => const Padding(padding: EdgeInsets.symmetric(vertical: 40),
          child: Center(child: CircularProgressIndicator())),
        error: (e, _) => Padding(padding: const EdgeInsets.symmetric(vertical: 24),
          child: Text('Failed to load: $e')),
        data: (a) => Column(mainAxisSize: MainAxisSize.min,
          crossAxisAlignment: CrossAxisAlignment.start, children: [
          Text(a.name, style: Theme.of(context).textTheme.headlineSmall),
          if (a.description != null && a.description!.isNotEmpty) ...[
            const SizedBox(height: 4),
            Text(a.description!, style: Theme.of(context).textTheme.bodyMedium),
          ],
          const SizedBox(height: 12),
          _row('Distance', '${a.details.distanceMeters} m'),
          _row('Ascent / descent', '${a.details.ascentMeters} m / ${a.details.descentMeters} m'),
          _row('Elevation', '${a.details.elevationMinMeters} – ${a.details.elevationMaxMeters} m'),
          _row('Difficulty', a.details.difficulty.name),
          _row('Surface', a.details.surface.name),
          _row('Marking', a.details.marking.name),
          if (a.details.estimatedHours != null)
            _row('Est. time', '${a.details.estimatedHours} h'),
          if (a.details.hasWaterSources) _row('Water', 'Available on trail'),
          if (a.details.hasShelter) _row('Shelter', 'Available'),
          if (a.details.waterSources.isNotEmpty) ...[
            const SizedBox(height: 8),
            Text('Water sources', style: Theme.of(context).textTheme.titleSmall),
            ...a.details.waterSources.map((w) =>
              Text('• ${w.kind}${w.notes != null ? " — ${w.notes}" : ""}')),
          ],
          HikingConditionsPanel(activityId: a.id),
          const SizedBox(height: 8),
          SheetActionBar(actions: [
            SheetAction(
              icon: Icons.close,
              label: 'Close',
              onPressed: () => Navigator.of(context).pop()),
            SheetAction(
              icon: Icons.edit_outlined,
              label: 'Edit',
              onPressed: () => _openEdit(context, a)),
            SheetAction(
              icon: Icons.delete_outline,
              label: 'Delete',
              onPressed: () => _confirmDelete(context, ref, a.id, a.name),
              isDestructive: true),
          ]),
        ]),
      ),
    ));
  }

  Widget _row(String label, String value) => Padding(
    padding: const EdgeInsets.symmetric(vertical: 4),
    child: Row(crossAxisAlignment: CrossAxisAlignment.start, children: [
      SizedBox(width: 132, child: Text(label)),
      Expanded(child: Text(value)),
    ]));

  void _openEdit(BuildContext context, HikingActivity a) {
    Navigator.of(context).pop();
    Navigator.of(context).push(
      MaterialPageRoute(
        builder: (_) => HikingCreateScreen(
          seedGeometry: ActivityGeometry.fromRoute(a.route),
          existing: a,
        ),
      ),
    );
  }

  Future<void> _confirmDelete(BuildContext context, WidgetRef ref, String id, String name) async {
    final confirmed = await showDialog<bool>(context: context,
      builder: (ctx) => AlertDialog(
        title: const Text('Delete hike?'),
        content: Text('"$name" will be removed.'),
        actions: [
          TextButton(onPressed: () => Navigator.of(ctx).pop(false), child: const Text('Cancel')),
          FilledButton(
            style: FilledButton.styleFrom(backgroundColor: Theme.of(ctx).colorScheme.error),
            onPressed: () => Navigator.of(ctx).pop(true), child: const Text('Delete')),
        ]));
    if (confirmed != true) return;
    try {
      await ref.read(hikingRepositoryProvider).delete(id);
      if (context.mounted) Navigator.of(context).pop();
    } catch (e) {
      if (context.mounted) {
        ScaffoldMessenger.of(context).showSnackBar(SnackBar(content: Text('Delete failed: $e')));
      }
    }
  }
}
