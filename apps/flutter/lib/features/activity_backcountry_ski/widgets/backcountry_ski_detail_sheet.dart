import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import 'package:turbo/features/activities/api.dart' show ActivityGeometry;

import '../data/backcountry_ski_repository.dart';
import '../models/backcountry_ski_activity.dart';
import '../models/backcountry_ski_details.dart';
import 'backcountry_ski_conditions_panel.dart';
import 'backcountry_ski_create_screen.dart';

class BackcountrySkiDetailSheet extends ConsumerWidget {
  final String activityId;
  const BackcountrySkiDetailSheet({super.key, required this.activityId});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final async = ref.watch(backcountrySkiActivityProvider(activityId));
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
              _row('Distance', '${a.details.distanceMeters} m'),
              _row('Ascent / descent',
                  '${a.details.ascentMeters} m / ${a.details.descentMeters} m'),
              _row('Elevation',
                  '${a.details.elevationMinMeters} – ${a.details.elevationMaxMeters} m'),
              _row('ATES', _atesLabel(a.details.atesRating)),
              if (a.details.dominantAspect != null)
                _row('Dominant aspect',
                    a.details.dominantAspect!.name.toUpperCase()),
              if (a.details.varsomRegionId != null)
                _row('Varsom region', a.details.varsomRegionId.toString()),
              if (a.details.aspectMix.isNotEmpty) ...[
                const SizedBox(height: 12),
                Text('Aspect mix',
                    style: Theme.of(context).textTheme.titleSmall),
                ...a.details.aspectMix.map(
                  (m) => Text(
                      '• ${m.aspect.name.toUpperCase()}: ${(m.fraction * 100).round()}%'),
                ),
              ],
              if (a.details.legs.isNotEmpty) ...[
                const SizedBox(height: 12),
                Text('Legs', style: Theme.of(context).textTheme.titleSmall),
                ...a.details.legs.asMap().entries.map(
                      (e) => Text(
                          '• ${e.key + 1}. ${_legKindLabel(e.value.kind)} '
                          '(${e.value.startElevationMeters} → ${e.value.endElevationMeters} m)'),
                    ),
              ],
              BackcountrySkiConditionsPanel(activityId: a.id),
              const SizedBox(height: 8),
              Row(
                children: [
                  TextButton.icon(
                    onPressed: () => Navigator.of(context).pop(),
                    icon: const Icon(Icons.close),
                    label: const Text('Close'),
                  ),
                  const Spacer(),
                  TextButton.icon(
                    onPressed: () => _openEdit(context, a),
                    icon: const Icon(Icons.edit_outlined),
                    label: const Text('Edit'),
                  ),
                  TextButton.icon(
                    style: TextButton.styleFrom(
                        foregroundColor: Theme.of(context).colorScheme.error),
                    onPressed: () => _confirmDelete(context, ref, a.id, a.name),
                    icon: const Icon(Icons.delete_outline),
                    label: const Text('Delete'),
                  ),
                ],
              ),
            ],
          ),
        ),
      ),
    );
  }

  Widget _row(String label, String value) => Padding(
        padding: const EdgeInsets.symmetric(vertical: 4),
        child: Row(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            SizedBox(width: 132, child: Text(label)),
            Expanded(child: Text(value)),
          ],
        ),
      );

  static String _atesLabel(AtesRating r) => switch (r) {
        AtesRating.unrated => 'Unrated',
        AtesRating.simple => 'Simple',
        AtesRating.challenging => 'Challenging',
        AtesRating.complex => 'Complex',
      };

  static String _legKindLabel(LegKind k) => switch (k) {
        LegKind.ascent => 'Ascent',
        LegKind.descent => 'Descent',
        LegKind.traverse => 'Traverse',
      };

  void _openEdit(BuildContext context, BackcountrySkiActivity a) {
    Navigator.of(context).pop();
    Navigator.of(context).push(
      MaterialPageRoute(
        builder: (_) => BackcountrySkiCreateScreen(
          seedGeometry: ActivityGeometry.fromRoute(a.route),
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
        title: const Text('Delete route?'),
        content: Text('"$name" will be removed.'),
        actions: [
          TextButton(
              onPressed: () => Navigator.of(ctx).pop(false),
              child: const Text('Cancel')),
          FilledButton(
            style: FilledButton.styleFrom(
                backgroundColor: Theme.of(ctx).colorScheme.error),
            onPressed: () => Navigator.of(ctx).pop(true),
            child: const Text('Delete'),
          ),
        ],
      ),
    );
    if (confirmed != true) return;
    try {
      await ref.read(backcountrySkiRepositoryProvider).delete(id);
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
