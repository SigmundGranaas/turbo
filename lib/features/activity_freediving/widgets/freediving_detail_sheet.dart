import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../data/freediving_repository.dart';

class FreedivingDetailSheet extends ConsumerWidget {
  final String activityId;
  const FreedivingDetailSheet({super.key, required this.activityId});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final async = ref.watch(freedivingActivityProvider(activityId));
    return SafeArea(child: Padding(padding: const EdgeInsets.all(16),
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
          _row('Water body', a.details.waterBody.name),
          _row('Bottom', a.details.bottomType.name),
          _row('Max depth', '${a.details.maxDepthMeters} m'),
          if (a.details.typicalVisibilityMeters != null)
            _row('Typical vis.', '${a.details.typicalVisibilityMeters} m'),
          _row('Shore entry', a.details.shoreEntry ? 'Yes' : 'No'),
          if (a.details.harpoonAllowed) _row('Harpoon', 'Allowed'),
          if (a.details.accessNotes != null && a.details.accessNotes!.isNotEmpty)
            _row('Access notes', a.details.accessNotes!),
          _row('Position',
            '${a.position.latitude.toStringAsFixed(5)}, ${a.position.longitude.toStringAsFixed(5)}'),
          if (a.details.targetSpecies.isNotEmpty) ...[
            const SizedBox(height: 8),
            Text('Target species', style: Theme.of(context).textTheme.titleSmall),
            ...a.details.targetSpecies.map((t) =>
              Text('• ${t.speciesCode}${t.notes != null ? " — ${t.notes}" : ""}')),
          ],
          const SizedBox(height: 16),
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

  Widget _row(String label, String value) => Padding(padding: const EdgeInsets.symmetric(vertical: 4),
    child: Row(crossAxisAlignment: CrossAxisAlignment.start, children: [
      SizedBox(width: 132, child: Text(label)),
      Expanded(child: Text(value)),
    ]));

  Future<void> _confirmDelete(BuildContext context, WidgetRef ref, String id, String name) async {
    final ok = await showDialog<bool>(context: context, builder: (ctx) => AlertDialog(
      title: const Text('Delete spot?'), content: Text('"$name" will be removed.'),
      actions: [
        TextButton(onPressed: () => Navigator.of(ctx).pop(false), child: const Text('Cancel')),
        FilledButton(
          style: FilledButton.styleFrom(backgroundColor: Theme.of(ctx).colorScheme.error),
          onPressed: () => Navigator.of(ctx).pop(true), child: const Text('Delete')),
      ]));
    if (ok != true) return;
    try {
      await ref.read(freedivingRepositoryProvider).delete(id);
      if (context.mounted) Navigator.of(context).pop();
    } catch (e) {
      if (context.mounted) ScaffoldMessenger.of(context).showSnackBar(SnackBar(content: Text('Delete failed: $e')));
    }
  }
}
