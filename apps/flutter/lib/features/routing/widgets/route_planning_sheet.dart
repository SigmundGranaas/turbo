import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../data/route_planning_notifier.dart';
import '../data/route_planning_state.dart';
import '../models/route_models.dart';
import '../providers/routing_providers.dart';

/// The center-bottom planning surface: preset chips, the live solve
/// summary (distance / time / ascent / surface mix), and edit actions.
/// Material 3, sized for a phone and capped for tablets by the caller.
class RoutePlanningSheet extends ConsumerWidget {
  const RoutePlanningSheet({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final state = ref.watch(routePlanningProvider);
    final notifier = ref.read(routePlanningProvider.notifier);
    final presetsAsync = ref.watch(routePresetsProvider);

    return Card(
      elevation: 4,
      margin: EdgeInsets.zero,
      shape: const RoundedRectangleBorder(
        borderRadius: BorderRadius.vertical(top: Radius.circular(24)),
      ),
      clipBehavior: Clip.antiAlias,
      child: Column(
        mainAxisSize: MainAxisSize.min,
        children: [
          // A thin progress hairline so the live re-solve feels responsive.
          SizedBox(
            height: 3,
            child: state.isPlanning
                ? const LinearProgressIndicator(minHeight: 3)
                : null,
          ),
          Padding(
            padding: const EdgeInsets.fromLTRB(16, 12, 16, 16),
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              mainAxisSize: MainAxisSize.min,
              children: [
                _PresetChips(
                  presetsAsync: presetsAsync,
                  selected: state.presetName,
                  onSelected: notifier.setPreset,
                ),
                const SizedBox(height: 12),
                _Body(state: state),
                const SizedBox(height: 4),
                _Actions(
                  canUndo: !state.isEmpty,
                  canClear: !state.isEmpty,
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

class _PresetChips extends StatelessWidget {
  final AsyncValue<List<RoutePreset>> presetsAsync;
  final String selected;
  final ValueChanged<String> onSelected;

  const _PresetChips({
    required this.presetsAsync,
    required this.selected,
    required this.onSelected,
  });

  @override
  Widget build(BuildContext context) {
    final presets = presetsAsync.value;
    if (presets == null || presets.isEmpty) {
      // Loading / unavailable: keep layout height stable.
      return const SizedBox(
        height: 36,
        child: Align(
          alignment: Alignment.centerLeft,
          child: Text('Loading presets…'),
        ),
      );
    }
    return SizedBox(
      height: 40,
      child: ListView.separated(
        scrollDirection: Axis.horizontal,
        itemCount: presets.length,
        separatorBuilder: (_, _) => const SizedBox(width: 8),
        itemBuilder: (context, i) {
          final p = presets[i];
          return ChoiceChip(
            label: Text(p.label),
            selected: p.name == selected,
            onSelected: (_) => onSelected(p.name),
            tooltip: p.description,
          );
        },
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
          Icon(Icons.error_outline, color: theme.colorScheme.error, size: 20),
          const SizedBox(width: 8),
          Expanded(
            child: Text(
              error,
              style: TextStyle(color: theme.colorScheme.error),
            ),
          ),
        ],
      );
    }

    if (state.isEmpty) {
      return Row(
        children: [
          Icon(Icons.touch_app_outlined,
              size: 20, color: theme.colorScheme.onSurfaceVariant),
          const SizedBox(width: 8),
          Expanded(
            child: Text('Tap the map to add stops',
                style: theme.textTheme.bodyMedium),
          ),
        ],
      );
    }

    if (!state.canPlan) {
      return Text('Add one more stop to plan a route',
          style: theme.textTheme.bodyMedium);
    }

    final plan = state.plan;
    if (plan == null) {
      // Have ≥2 stops but no result yet (first solve in flight).
      return Text('Planning…', style: theme.textTheme.bodyMedium);
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
          children: [
            _Stat(icon: Icons.straighten, label: _distance(plan.distanceM)),
            const SizedBox(width: 20),
            _Stat(icon: Icons.schedule, label: _duration(plan.duration)),
            const SizedBox(width: 20),
            _Stat(icon: Icons.terrain, label: '↑ ${plan.ascentM.round()} m'),
          ],
        ),
        if (plan.surfaces.isNotEmpty) ...[
          const SizedBox(height: 10),
          Text(
            _surfaceLine(plan),
            style: theme.textTheme.bodySmall
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
    final parts = entries.take(3).map((e) {
      final pct = (e.value / total * 100).round();
      return '$pct% ${labels[e.key] ?? e.key}';
    });
    return parts.join(' · ');
  }
}

class _Stat extends StatelessWidget {
  final IconData icon;
  final String label;
  const _Stat({required this.icon, required this.label});

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return Row(
      mainAxisSize: MainAxisSize.min,
      children: [
        Icon(icon, size: 18, color: theme.colorScheme.primary),
        const SizedBox(width: 6),
        Text(label,
            style: theme.textTheme.titleMedium
                ?.copyWith(fontWeight: FontWeight.w600)),
      ],
    );
  }
}

class _Actions extends StatelessWidget {
  final bool canUndo;
  final bool canClear;
  final VoidCallback onUndo;
  final VoidCallback onClear;

  const _Actions({
    required this.canUndo,
    required this.canClear,
    required this.onUndo,
    required this.onClear,
  });

  @override
  Widget build(BuildContext context) {
    return Row(
      mainAxisAlignment: MainAxisAlignment.end,
      children: [
        TextButton.icon(
          onPressed: canUndo ? onUndo : null,
          icon: const Icon(Icons.undo, size: 18),
          label: const Text('Undo'),
        ),
        const SizedBox(width: 4),
        TextButton.icon(
          onPressed: canClear ? onClear : null,
          icon: const Icon(Icons.clear, size: 18),
          label: const Text('Clear'),
        ),
      ],
    );
  }
}
