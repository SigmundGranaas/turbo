import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import 'package:turbo/core/widgets/sheet_action_bar.dart';
import 'package:turbo/features/activities/api.dart' show ActivityGeometry;

import '../data/fishing_repository.dart';
import '../models/fishing_activity.dart';
import '../models/fishing_details.dart';
import 'fishing_conditions_panel.dart';
import 'fishing_create_screen.dart';

/// Read-only detail surface for a fishing activity. The typed model
/// drives the layout — no string-keyed map lookups.
class FishingDetailSheet extends ConsumerWidget {
  final String activityId;
  const FishingDetailSheet({super.key, required this.activityId});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final async = ref.watch(fishingActivityProvider(activityId));
    return SafeArea(
      child: Padding(
        padding: const EdgeInsets.all(16),
        child: async.when(
          loading: () => const Padding(
            padding: EdgeInsets.symmetric(vertical: 40),
            child: Center(child: CircularProgressIndicator()),
          ),
          error: (e, _) => Padding(
            padding: const EdgeInsets.symmetric(vertical: 24),
            child: Text('Failed to load: $e'),
          ),
          data: (a) => Column(
            mainAxisSize: MainAxisSize.min,
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Text(a.name, style: Theme.of(context).textTheme.headlineSmall),
              if (a.description != null && a.description!.isNotEmpty) ...[
                const SizedBox(height: 4),
                Text(a.description!, style: Theme.of(context).textTheme.bodyMedium),
              ],
              const SizedBox(height: 12),
              _Row(label: 'Water', value: _waterLabel(a.details.waterKind)),
              _Row(label: 'Access', value: _accessLabel(a.details.shoreOrBoat)),
              if (a.details.accessNotes != null && a.details.accessNotes!.isNotEmpty)
                _Row(label: 'Notes', value: a.details.accessNotes!),
              _Row(
                label: 'Position',
                value: '${a.position.latitude.toStringAsFixed(5)}, '
                    '${a.position.longitude.toStringAsFixed(5)}',
              ),
              if (a.details.targetSpecies.isNotEmpty) ...[
                const SizedBox(height: 12),
                Text('Target species',
                    style: Theme.of(context).textTheme.titleSmall),
                ...a.details.targetSpecies.map(
                  (s) => Text('• ${s.speciesCode}${s.notes != null ? " — ${s.notes}" : ""}'),
                ),
              ],
              FishingConditionsPanel(activityId: a.id),
              const SizedBox(height: 8),
              SheetActionBar(
                actions: [
                  SheetAction(
                    icon: Icons.close,
                    label: 'Close',
                    onPressed: () => Navigator.of(context).pop(),
                  ),
                  SheetAction(
                    icon: Icons.edit_outlined,
                    label: 'Edit',
                    onPressed: () => _openEdit(context, a),
                  ),
                  SheetAction(
                    icon: Icons.delete_outline,
                    label: 'Delete',
                    onPressed: () => _confirmDelete(context, ref, a.id, a.name),
                    isDestructive: true,
                  ),
                ],
              ),
            ],
          ),
        ),
      ),
    );
  }

  static String _waterLabel(WaterKind w) => switch (w) {
        WaterKind.river => 'River',
        WaterKind.lake => 'Lake',
        WaterKind.sea => 'Sea',
      };

  static String _accessLabel(ShoreOrBoat s) => switch (s) {
        ShoreOrBoat.shore => 'Shore',
        ShoreOrBoat.boat => 'Boat',
        ShoreOrBoat.either => 'Either',
      };

  void _openEdit(BuildContext context, FishingActivity a) {
    Navigator.of(context).pop(); // close the sheet
    Navigator.of(context).push(
      MaterialPageRoute(
        builder: (_) => FishingCreateScreen(
          seedGeometry: ActivityGeometry.fromPoint(a.position),
          existing: a,
        ),
      ),
    );
  }

  Future<void> _confirmDelete(
      BuildContext context, WidgetRef ref, String id, String name) async {
    final confirmed = await showDialog<bool>(
      context: context,
      builder: (ctx) => AlertDialog(
        title: const Text('Delete spot?'),
        content: Text('"$name" will be removed.'),
        actions: [
          TextButton(onPressed: () => Navigator.of(ctx).pop(false), child: const Text('Cancel')),
          FilledButton(
            style: FilledButton.styleFrom(backgroundColor: Theme.of(ctx).colorScheme.error),
            onPressed: () => Navigator.of(ctx).pop(true),
            child: const Text('Delete'),
          ),
        ],
      ),
    );
    if (confirmed != true) return;
    try {
      await ref.read(fishingRepositoryProvider).delete(id);
      if (context.mounted) Navigator.of(context).pop();
    } catch (e) {
      if (context.mounted) {
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(content: Text('Delete failed: $e')),
        );
      }
    }
  }
}

class _Row extends StatelessWidget {
  final String label;
  final String value;
  const _Row({required this.label, required this.value});

  @override
  Widget build(BuildContext context) => Padding(
        padding: const EdgeInsets.symmetric(vertical: 4),
        child: Row(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            SizedBox(
              width: 88,
              child: Text(label, style: Theme.of(context).textTheme.labelMedium),
            ),
            Expanded(child: Text(value)),
          ],
        ),
      );
}
