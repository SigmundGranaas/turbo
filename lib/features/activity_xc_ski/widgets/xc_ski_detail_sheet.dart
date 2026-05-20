import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../data/xc_ski_repository.dart';
import 'xc_ski_conditions_panel.dart';

class XcSkiDetailSheet extends ConsumerWidget {
  final String activityId;
  const XcSkiDetailSheet({super.key, required this.activityId});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final async = ref.watch(xcSkiActivityProvider(activityId));
    return SafeArea(child: Padding(
      padding: const EdgeInsets.all(16),
      child: async.when(
        loading: () => const Padding(padding: EdgeInsets.symmetric(vertical: 40),
          child: Center(child: CircularProgressIndicator())),
        error: (e, _) => Text('Failed to load: $e'),
        data: (a) => Column(mainAxisSize: MainAxisSize.min, crossAxisAlignment: CrossAxisAlignment.start, children: [
          Text(a.name, style: Theme.of(context).textTheme.headlineSmall),
          if (a.description != null && a.description!.isNotEmpty) ...[
            const SizedBox(height: 4),
            Text(a.description!, style: Theme.of(context).textTheme.bodyMedium),
          ],
          const SizedBox(height: 12),
          _row('Distance', '${a.details.distanceMeters} m'),
          _row('Ascent / descent', '${a.details.ascentMeters} m / ${a.details.descentMeters} m'),
          _row('Technique', a.details.technique.name),
          _row('Grooming', a.details.groomingStatus.name),
          if (a.details.isLit) _row('Lit', 'Yes'),
          if (a.details.requiresSeasonPass) _row('Season pass', 'Required'),
          XcSkiConditionsPanel(activityId: a.id),
          const SizedBox(height: 8),
          Row(children: [
            TextButton.icon(onPressed: () => Navigator.of(context).pop(),
              icon: const Icon(Icons.close), label: const Text('Close')),
            const Spacer(),
            TextButton.icon(
              style: TextButton.styleFrom(foregroundColor: Theme.of(context).colorScheme.error),
              onPressed: () => _confirmDelete(context, ref, a.id, a.name),
              icon: const Icon(Icons.delete_outline), label: const Text('Delete')),
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

  Future<void> _confirmDelete(BuildContext context, WidgetRef ref, String id, String name) async {
    final ok = await showDialog<bool>(context: context, builder: (ctx) => AlertDialog(
      title: const Text('Delete trail?'), content: Text('"$name" will be removed.'),
      actions: [
        TextButton(onPressed: () => Navigator.of(ctx).pop(false), child: const Text('Cancel')),
        FilledButton(
          style: FilledButton.styleFrom(backgroundColor: Theme.of(ctx).colorScheme.error),
          onPressed: () => Navigator.of(ctx).pop(true), child: const Text('Delete')),
      ]));
    if (ok != true) return;
    try {
      await ref.read(xcSkiRepositoryProvider).delete(id);
      if (context.mounted) Navigator.of(context).pop();
    } catch (e) {
      if (context.mounted) ScaffoldMessenger.of(context).showSnackBar(SnackBar(content: Text('Delete failed: $e')));
    }
  }
}
