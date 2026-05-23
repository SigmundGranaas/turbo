import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import 'package:turbo/features/activities/api.dart' show ActivityGeometry;

import 'packrafting_conditions_panel.dart';
import 'packrafting_create_screen.dart';

import '../data/packrafting_repository.dart';
import '../models/packrafting_activity.dart';
import '../models/packrafting_details.dart';

class PackraftingDetailSheet extends ConsumerWidget {
  final String activityId;
  const PackraftingDetailSheet({super.key, required this.activityId});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final async = ref.watch(packraftingActivityProvider(activityId));
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
          _row('Distance', '${a.details.distanceMeters} m'),
          _row('Paddle / portage', '${a.details.paddleDistanceMeters} m / ${a.details.portageDistanceMeters} m'),
          _row('Typical / max grade', '${_g(a.details.typicalGrade)} / ${_g(a.details.maxGrade)}'),
          _row('Put-in', '${a.details.putIn.latitude.toStringAsFixed(5)}, ${a.details.putIn.longitude.toStringAsFixed(5)}'),
          _row('Take-out', '${a.details.takeOut.latitude.toStringAsFixed(5)}, ${a.details.takeOut.longitude.toStringAsFixed(5)}'),
          if (a.details.nveStationCode != null) _row('NVE station', a.details.nveStationCode!),
          if (a.details.minFlowCumecs != null || a.details.maxFlowCumecs != null)
            _row('Flow window',
              '${a.details.minFlowCumecs ?? "?"} – ${a.details.maxFlowCumecs ?? "?"} m³/s'),
          if (a.details.segments.isNotEmpty) ...[
            const SizedBox(height: 12),
            Text('Segments', style: Theme.of(context).textTheme.titleSmall),
            ...a.details.segments.asMap().entries.map((e) => Text(
              '• ${e.key + 1}. ${e.value.kind.name}'
              '${e.value.grade != null ? " (${_g(e.value.grade!)})" : ""}'
              ' — ${e.value.distanceMeters} m')),
          ],
          PackraftingConditionsPanel(activityId: a.id),
          const SizedBox(height: 8),
          Row(children: [
            TextButton.icon(onPressed: () => Navigator.of(context).pop(),
              icon: const Icon(Icons.close), label: const Text('Close')),
            const Spacer(),
            TextButton.icon(
              onPressed: () => _openEdit(context, a),
              icon: const Icon(Icons.edit_outlined),
              label: const Text('Edit')),
            TextButton.icon(
              style: TextButton.styleFrom(foregroundColor: Theme.of(context).colorScheme.error),
              onPressed: () => _confirmDelete(context, ref, a.id, a.name),
              icon: const Icon(Icons.delete_outline), label: const Text('Delete')),
          ]),
        ]),
      ),
    ));
  }

  static String _g(WaterGrade g) => switch (g) {
    WaterGrade.flatwater => 'Flatwater',
    WaterGrade.i => 'I', WaterGrade.ii => 'II', WaterGrade.iii => 'III',
    WaterGrade.iv => 'IV', WaterGrade.v => 'V', WaterGrade.vi => 'VI',
  };

  Widget _row(String label, String value) => Padding(padding: const EdgeInsets.symmetric(vertical: 4),
    child: Row(crossAxisAlignment: CrossAxisAlignment.start, children: [
      SizedBox(width: 152, child: Text(label)),
      Expanded(child: Text(value)),
    ]));

  void _openEdit(BuildContext context, PackraftingActivity a) {
    Navigator.of(context).pop();
    Navigator.of(context).push(
      MaterialPageRoute(
        builder: (_) => PackraftingCreateScreen(
          seedGeometry: ActivityGeometry.fromRoute(a.route),
          existing: a,
        ),
      ),
    );
  }

  Future<void> _confirmDelete(BuildContext context, WidgetRef ref, String id, String name) async {
    final ok = await showDialog<bool>(context: context, builder: (ctx) => AlertDialog(
      title: const Text('Delete trip?'), content: Text('"$name" will be removed.'),
      actions: [
        TextButton(onPressed: () => Navigator.of(ctx).pop(false), child: const Text('Cancel')),
        FilledButton(
          style: FilledButton.styleFrom(backgroundColor: Theme.of(ctx).colorScheme.error),
          onPressed: () => Navigator.of(ctx).pop(true), child: const Text('Delete')),
      ]));
    if (ok != true) return;
    try {
      await ref.read(packraftingRepositoryProvider).delete(id);
      if (context.mounted) Navigator.of(context).pop();
    } catch (e) {
      if (context.mounted) ScaffoldMessenger.of(context).showSnackBar(SnackBar(content: Text('Delete failed: $e')));
    }
  }
}
