import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:turbo/core/widgets/app_snackbars.dart';
import 'package:turbo/features/saved_paths/api.dart';

import '../data/route_planning_notifier.dart';
import '../data/route_planning_state.dart';
import '../models/route_models.dart';
import '../providers/routing_providers.dart';

/// The center-bottom planning surface: the live solve summary up top, a
/// native route-style row, and the actions (undo / clear / save as track).
class RoutePlanningSheet extends ConsumerWidget {
  const RoutePlanningSheet({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final state = ref.watch(routePlanningProvider);
    final notifier = ref.read(routePlanningProvider.notifier);
    final presets = ref.watch(routePresetsProvider).value ?? const [];

    return Card(
      elevation: 4,
      margin: EdgeInsets.zero,
      shape: const RoundedRectangleBorder(
        borderRadius: BorderRadius.vertical(top: Radius.circular(28)),
      ),
      clipBehavior: Clip.antiAlias,
      child: Column(
        mainAxisSize: MainAxisSize.min,
        children: [
          SizedBox(
            height: 3,
            child: state.isPlanning
                ? const LinearProgressIndicator(minHeight: 3)
                : null,
          ),
          Padding(
            padding: const EdgeInsets.fromLTRB(20, 16, 12, 12),
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              mainAxisSize: MainAxisSize.min,
              children: [
                Padding(
                  padding: const EdgeInsets.only(right: 8),
                  child: _Body(state: state),
                ),
                const SizedBox(height: 8),
                _RouteStyleRow(
                  presets: presets,
                  selected: state.presetName,
                  onSelected: notifier.setPreset,
                ),
                _ActionRow(
                  state: state,
                  onUndo: notifier.undoLast,
                  onClear: notifier.clear,
                  onSave: () => _saveAsTrack(context, state),
                ),
              ],
            ),
          ),
        ],
      ),
    );
  }

  Future<void> _saveAsTrack(BuildContext context, RoutePlanningState state) async {
    final plan = state.plan;
    if (plan == null) return;
    final saved = await showModalBottomSheet<bool>(
      context: context,
      isScrollControlled: true,
      useSafeArea: true,
      builder: (_) => SavePathSheet(
        points: plan.geometry,
        distance: plan.distanceM,
        ascent: plan.ascentM,
      ),
    );
    if (saved == true && context.mounted) {
      AppSnackbars.success(context, 'Route saved as a track');
    }
  }
}

/// Native tappable row (settings-idiom): leading icon, label, current
/// value + chevron. Opens a bottom-sheet picker — the app's standard
/// choose-one pattern.
class _RouteStyleRow extends StatelessWidget {
  final List<RoutePreset> presets;
  final String selected;
  final ValueChanged<String> onSelected;

  const _RouteStyleRow({
    required this.presets,
    required this.selected,
    required this.onSelected,
  });

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    RoutePreset? current;
    for (final p in presets) {
      if (p.name == selected) {
        current = p;
        break;
      }
    }
    return InkWell(
      borderRadius: BorderRadius.circular(12),
      onTap: presets.isEmpty ? null : () => _openPicker(context),
      child: Padding(
        padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 10),
        child: Row(
          children: [
            Icon(Icons.tune, size: 20, color: theme.colorScheme.onSurfaceVariant),
            const SizedBox(width: 12),
            Text('Route style', style: theme.textTheme.bodyLarge),
            const Spacer(),
            Text(
              current?.label ?? 'Balanced',
              style: theme.textTheme.bodyLarge?.copyWith(
                color: theme.colorScheme.primary,
                fontWeight: FontWeight.w600,
              ),
            ),
            Icon(Icons.arrow_drop_down, color: theme.colorScheme.onSurfaceVariant),
          ],
        ),
      ),
    );
  }

  void _openPicker(BuildContext context) {
    showModalBottomSheet<void>(
      context: context,
      showDragHandle: true,
      builder: (sheetCtx) => SafeArea(
        child: Column(
          mainAxisSize: MainAxisSize.min,
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            Padding(
              padding: const EdgeInsets.fromLTRB(20, 4, 20, 12),
              child: Text('Route style',
                  style: Theme.of(sheetCtx).textTheme.titleLarge),
            ),
            for (final p in presets)
              ListTile(
                title: Text(p.label),
                subtitle: Text(p.description),
                trailing: p.name == selected
                    ? Icon(Icons.check, color: Theme.of(sheetCtx).colorScheme.primary)
                    : null,
                onTap: () {
                  onSelected(p.name);
                  Navigator.of(sheetCtx).pop();
                },
              ),
            const SizedBox(height: 8),
          ],
        ),
      ),
    );
  }
}

class _Body extends StatelessWidget {
  final RoutePlanningState state;
  const _Body({required this.state});

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);

    final error = state.error;
    if (error != null) {
      return Row(
        children: [
          Icon(Icons.error_outline, color: theme.colorScheme.error, size: 22),
          const SizedBox(width: 10),
          Expanded(
            child: Text(error, style: TextStyle(color: theme.colorScheme.error)),
          ),
        ],
      );
    }

    if (state.isEmpty || !state.canPlan) {
      return Align(
        alignment: Alignment.centerLeft,
        child: Text(
          state.isEmpty ? 'Tap the map to add stops' : 'Add one more stop to plan a route',
          style: theme.textTheme.bodyLarge
              ?.copyWith(color: theme.colorScheme.onSurfaceVariant),
        ),
      );
    }

    final plan = state.plan;
    if (plan == null) {
      return Align(
        alignment: Alignment.centerLeft,
        child: Text('Planning…',
            style: theme.textTheme.bodyLarge
                ?.copyWith(color: theme.colorScheme.onSurfaceVariant)),
      );
    }

    return _Summary(plan: plan);
  }
}

class _Summary extends StatelessWidget {
  final RoutePlan plan;
  const _Summary({required this.plan});

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Row(
          mainAxisAlignment: MainAxisAlignment.spaceBetween,
          children: [
            _Stat(icon: Icons.straighten, value: _distance(plan.distanceM), label: 'distance'),
            _Stat(icon: Icons.schedule, value: _duration(plan.duration), label: 'time'),
            _Stat(icon: Icons.terrain, value: '${plan.ascentM.round()} m', label: 'ascent'),
          ],
        ),
        if (plan.surfaces.isNotEmpty) ...[
          const SizedBox(height: 12),
          Text(
            _surfaceLine(plan),
            style: theme.textTheme.bodyMedium
                ?.copyWith(color: theme.colorScheme.onSurfaceVariant),
          ),
        ],
      ],
    );
  }

  static String _distance(double m) =>
      m >= 1000 ? '${(m / 1000).toStringAsFixed(1)} km' : '${m.round()} m';

  static String _duration(Duration d) {
    final h = d.inHours;
    final min = d.inMinutes.remainder(60);
    if (h > 0) return '$h h ${min.toString().padLeft(2, '0')} min';
    return '$min min';
  }

  static String _surfaceLine(RoutePlan plan) {
    const labels = {
      'trail': 'trail',
      'road': 'road',
      'ski_track': 'ski track',
      'off_trail': 'off-trail',
      'unknown': 'unknown',
    };
    final total = plan.surfaces.values.fold<double>(0, (a, b) => a + b);
    if (total <= 0) return '${plan.onTrailPct.round()}% on trail';
    final entries = plan.surfaces.entries.toList()
      ..sort((a, b) => b.value.compareTo(a.value));
    return entries.take(3).map((e) {
      final pct = (e.value / total * 100).round();
      return '$pct% ${labels[e.key] ?? e.key}';
    }).join('  ·  ');
  }
}

class _Stat extends StatelessWidget {
  final IconData icon;
  final String value;
  final String label;
  const _Stat({required this.icon, required this.value, required this.label});

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Row(
          mainAxisSize: MainAxisSize.min,
          children: [
            Icon(icon, size: 20, color: theme.colorScheme.primary),
            const SizedBox(width: 6),
            Text(value,
                style: theme.textTheme.titleLarge
                    ?.copyWith(fontWeight: FontWeight.w600)),
          ],
        ),
        const SizedBox(height: 2),
        Text(label,
            style: theme.textTheme.labelSmall
                ?.copyWith(color: theme.colorScheme.onSurfaceVariant)),
      ],
    );
  }
}

class _ActionRow extends StatelessWidget {
  final RoutePlanningState state;
  final VoidCallback onUndo;
  final VoidCallback onClear;
  final VoidCallback onSave;

  const _ActionRow({
    required this.state,
    required this.onUndo,
    required this.onClear,
    required this.onSave,
  });

  @override
  Widget build(BuildContext context) {
    final hasStops = !state.isEmpty;
    final canSave = state.plan != null;
    return Row(
      children: [
        IconButton(
          tooltip: 'Undo last stop',
          onPressed: hasStops ? onUndo : null,
          icon: const Icon(Icons.undo),
        ),
        IconButton(
          tooltip: 'Clear route',
          onPressed: hasStops ? onClear : null,
          icon: const Icon(Icons.delete_outline),
        ),
        const Spacer(),
        FilledButton.icon(
          onPressed: canSave ? onSave : null,
          icon: const Icon(Icons.bookmark_add_outlined, size: 20),
          label: const Text('Save as track'),
        ),
      ],
    );
  }
}
