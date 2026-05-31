import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../data/route_planning_notifier.dart';
import '../data/route_planning_state.dart';
import '../models/route_models.dart';
import '../providers/routing_providers.dart';

/// The center-bottom planning surface. A single roomy "route style"
/// selector (presets live behind a picker, not a crowded chip row), the
/// live solve summary, and edit actions. Material 3, generous spacing.
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
            padding: const EdgeInsets.fromLTRB(20, 18, 20, 20),
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              mainAxisSize: MainAxisSize.min,
              children: [
                _PresetSelector(
                  presets: presets,
                  selected: state.presetName,
                  onSelected: notifier.setPreset,
                ),
                const SizedBox(height: 20),
                _Body(state: state),
                const SizedBox(height: 16),
                _Actions(
                  enabled: !state.isEmpty,
                  onUndo: notifier.undoLast,
                  onClear: notifier.clear,
                ),
              ],
            ),
          ),
        ],
      ),
    );
  }
}

class _PresetSelector extends StatelessWidget {
  final List<RoutePreset> presets;
  final String selected;
  final ValueChanged<String> onSelected;

  const _PresetSelector({
    required this.presets,
    required this.selected,
    required this.onSelected,
  });

  RoutePreset? get _current {
    for (final p in presets) {
      if (p.name == selected) return p;
    }
    return null;
  }

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final current = _current;
    final label = current?.label ?? 'Balanced';

    return InkWell(
      borderRadius: BorderRadius.circular(14),
      onTap: presets.isEmpty ? null : () => _openPicker(context),
      child: Container(
        padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 14),
        decoration: BoxDecoration(
          borderRadius: BorderRadius.circular(14),
          border: Border.all(color: theme.colorScheme.outlineVariant),
        ),
        child: Row(
          children: [
            Icon(Icons.tune, size: 22, color: theme.colorScheme.primary),
            const SizedBox(width: 14),
            Expanded(
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Text('Route style',
                      style: theme.textTheme.labelMedium?.copyWith(
                          color: theme.colorScheme.onSurfaceVariant)),
                  const SizedBox(height: 2),
                  Text(label,
                      style: theme.textTheme.titleMedium
                          ?.copyWith(fontWeight: FontWeight.w600)),
                ],
              ),
            ),
            Icon(Icons.expand_more, color: theme.colorScheme.onSurfaceVariant),
          ],
        ),
      ),
    );
  }

  void _openPicker(BuildContext context) {
    showModalBottomSheet<void>(
      context: context,
      showDragHandle: true,
      builder: (sheetCtx) {
        return SafeArea(
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
                      ? Icon(Icons.check,
                          color: Theme.of(sheetCtx).colorScheme.primary)
                      : null,
                  onTap: () {
                    onSelected(p.name);
                    Navigator.of(sheetCtx).pop();
                  },
                ),
              const SizedBox(height: 8),
            ],
          ),
        );
      },
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
            child: Text(error,
                style: TextStyle(color: theme.colorScheme.error)),
          ),
        ],
      );
    }

    if (state.isEmpty || !state.canPlan) {
      final msg = state.isEmpty
          ? 'Tap the map to add stops'
          : 'Add one more stop to plan a route';
      return Padding(
        padding: const EdgeInsets.symmetric(vertical: 4),
        child: Text(msg,
            style: theme.textTheme.bodyLarge
                ?.copyWith(color: theme.colorScheme.onSurfaceVariant)),
      );
    }

    final plan = state.plan;
    if (plan == null) {
      return Padding(
        padding: const EdgeInsets.symmetric(vertical: 4),
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
          const SizedBox(height: 14),
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

class _Actions extends StatelessWidget {
  final bool enabled;
  final VoidCallback onUndo;
  final VoidCallback onClear;

  const _Actions({
    required this.enabled,
    required this.onUndo,
    required this.onClear,
  });

  @override
  Widget build(BuildContext context) {
    return Row(
      mainAxisAlignment: MainAxisAlignment.end,
      children: [
        TextButton.icon(
          onPressed: enabled ? onUndo : null,
          icon: const Icon(Icons.undo, size: 20),
          label: const Text('Undo'),
        ),
        const SizedBox(width: 8),
        TextButton.icon(
          onPressed: enabled ? onClear : null,
          icon: const Icon(Icons.clear, size: 20),
          label: const Text('Clear'),
        ),
      ],
    );
  }
}
