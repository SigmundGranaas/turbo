import 'package:flutter/material.dart';
import 'package:turbo/core/widgets/exclusive_sheet.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:turbo/app/tokens.dart';
import 'package:turbo/core/widgets/app_button.dart';
import 'package:turbo/core/widgets/app_pill.dart';
import 'package:turbo/core/widgets/app_snackbars.dart';
import 'package:turbo/core/widgets/sheet_drag_handle.dart';
import 'package:turbo/features/saved_paths/api.dart';
import 'package:turbo/features/journey/api.dart';
import 'package:turbo/features/map_view/api.dart';

import '../data/route_planning_notifier.dart';
import '../data/route_planning_state.dart';
import '../models/route_models.dart';
import '../models/route_geo_path.dart';
import '../providers/routing_providers.dart';

/// Center-bottom planning panel. Built from the app's own map-control
/// components (AppCardSurface + AppButton + AppSpacing) so it matches the
/// measuring / download toolbars: an info+Save header row, a divider, then
/// the route-style selector and edit actions.
class RoutePlanningSheet extends ConsumerWidget {
  const RoutePlanningSheet({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final state = ref.watch(routePlanningProvider);
    final notifier = ref.read(routePlanningProvider.notifier);
    final presets = ref.watch(routePresetsProvider).value ?? const [];

    return AppCardSurface(
      margin: const EdgeInsets.symmetric(horizontal: AppSpacing.l),
      maxWidth: 460,
      child: Column(
        mainAxisSize: MainAxisSize.min,
        children: [
          const SheetDragHandle(),
          // Row 1: live summary + Save.
          Padding(
            padding: const EdgeInsets.fromLTRB(
                AppSpacing.l, AppSpacing.m, AppSpacing.l, AppSpacing.m),
            child: Row(
              children: [
                Expanded(child: _Info(state: state)),
                const SizedBox(width: AppSpacing.m),
                AppButton.tonal(
                  text: 'Save',
                  onPressed: state.plan != null
                      ? () => _saveAsTrack(context, state)
                      : null,
                ),
                const SizedBox(width: AppSpacing.s),
                AppButton.primary(
                  text: 'Follow',
                  onPressed: state.plan != null
                      ? () => _followRoute(context, ref, state)
                      : null,
                ),
              ],
            ),
          ),
          const Divider(
              height: 1, indent: AppSpacing.l, endIndent: AppSpacing.l),
          // Row 2: route-style selector (left) + edit actions (right).
          Padding(
            padding: const EdgeInsets.symmetric(
                horizontal: AppSpacing.s, vertical: AppSpacing.xs),
            child: Row(
              children: [
                _RouteStyleButton(
                  presets: presets,
                  selected: state.presetName,
                  onSelected: notifier.setPreset,
                ),
                const Spacer(),
                IconButton(
                  tooltip: 'Undo last stop',
                  onPressed: state.isEmpty ? null : notifier.undoLast,
                  icon: const Icon(Icons.undo),
                ),
                IconButton(
                  tooltip: 'Clear route',
                  onPressed: state.isEmpty ? null : notifier.clear,
                  icon: const Icon(Icons.delete_sweep_outlined),
                ),
              ],
            ),
          ),
        ],
      ),
    );
  }

  /// Start a live journey that follows the solved route, then return to the
  /// map so the user can watch their position track along it. This is the
  /// integration the old flow lacked — a planned route was a dead-end snapshot.
  /// Start following the planned route. Recording is opt-in from the
  /// active-outing panel ("Record this outing"); the route's waypoints ride
  /// along so the outing panel's Edit can reopen the planner.
  void _followRoute(
      BuildContext context, WidgetRef ref, RoutePlanningState state) {
    final plan = state.plan;
    if (plan == null) return;
    ref.read(activeJourneyProvider.notifier).followPath(
          plan.toGeoPath(),
          label: 'Planned route',
          waypoints: state.waypoints,
        );
    // Close the planning tool to reveal the live map following the route.
    ref.read(activeMapToolProvider.notifier).deactivate();
  }

  Future<void> _saveAsTrack(BuildContext context, RoutePlanningState state) async {
    final plan = state.plan;
    if (plan == null) return;
    final saved = await showExclusiveSheet<bool>(
      context,
      replace: false,
      builder: (_) => SavePathSheet.fromGeoPath(plan.toGeoPath()),
    );
    if (saved == true && context.mounted) {
      AppSnackbars.success(context, 'Route saved as a track');
    }
  }
}

/// The header content: a hint, the "planning…" state, an error, or the
/// solved summary (distance · time · ascent + surface mix).
class _Info extends StatelessWidget {
  final RoutePlanningState state;
  const _Info({required this.state});

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final scheme = theme.colorScheme;

    final error = state.error;
    if (error != null) {
      return Row(
        children: [
          Icon(Icons.error_outline, color: scheme.error, size: 20),
          const SizedBox(width: AppSpacing.s),
          Expanded(
            child: Text(error,
                style: theme.textTheme.bodyMedium
                    ?.copyWith(color: scheme.error)),
          ),
        ],
      );
    }

    if (state.isEmpty || !state.canPlan) {
      return _LeadingLine(
        icon: Icons.touch_app_outlined,
        text: state.isEmpty
            ? 'Tap the map to add stops'
            : 'Add one more stop',
      );
    }

    final plan = state.plan;
    if (plan == null) {
      return Row(
        children: [
          SizedBox(
            width: 16,
            height: 16,
            child: CircularProgressIndicator(
                strokeWidth: 2, color: scheme.primary),
          ),
          const SizedBox(width: AppSpacing.m),
          Text('Planning…',
              style: theme.textTheme.titleMedium
                  ?.copyWith(color: scheme.onSurfaceVariant)),
        ],
      );
    }

    return Row(
      children: [
        Icon(Icons.route_outlined, color: scheme.onSurfaceVariant),
        const SizedBox(width: AppSpacing.m),
        Expanded(
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            mainAxisSize: MainAxisSize.min,
            children: [
              Text(
                '${_distance(plan.distanceM)}  ·  ${_duration(plan.duration)}  ·  ↑${plan.ascentM.round()} m',
                style: theme.textTheme.titleMedium
                    ?.copyWith(fontWeight: FontWeight.bold),
              ),
              if (plan.surfaces.isNotEmpty)
                Text(_surfaceLine(plan),
                    style: theme.textTheme.bodySmall
                        ?.copyWith(color: scheme.onSurfaceVariant)),
            ],
          ),
        ),
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

class _LeadingLine extends StatelessWidget {
  final IconData icon;
  final String text;
  const _LeadingLine({required this.icon, required this.text});

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return Row(
      children: [
        Icon(icon, color: theme.colorScheme.onSurfaceVariant),
        const SizedBox(width: AppSpacing.m),
        Expanded(
          child: Text(text,
              style: theme.textTheme.titleMedium
                  ?.copyWith(color: theme.colorScheme.onSurfaceVariant)),
        ),
      ],
    );
  }
}

/// Route-style selector — a tonal text button showing the current style
/// with a dropdown affordance; opens the standard bottom-sheet picker.
class _RouteStyleButton extends StatelessWidget {
  final List<RoutePreset> presets;
  final String selected;
  final ValueChanged<String> onSelected;

  const _RouteStyleButton({
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
    return TextButton.icon(
      onPressed: presets.isEmpty ? null : () => _openPicker(context),
      icon: const Icon(Icons.tune, size: 20),
      label: Row(
        mainAxisSize: MainAxisSize.min,
        children: [
          Text(current?.label ?? 'Balanced'),
          Icon(Icons.arrow_drop_down, color: theme.colorScheme.onSurfaceVariant),
        ],
      ),
    );
  }

  void _openPicker(BuildContext context) {
    showExclusiveSheet<void>(
      context,
      replace: false,
      builder: (sheetCtx) {
        final theme = Theme.of(sheetCtx);
        return SafeArea(
          child: SingleChildScrollView(
            child: Padding(
              padding: const EdgeInsets.fromLTRB(
                  AppSpacing.l, 0, AppSpacing.l, AppSpacing.l),
              child: Column(
                mainAxisSize: MainAxisSize.min,
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Row(
                    children: [
                      Icon(Icons.tune, color: theme.colorScheme.primary),
                      const SizedBox(width: AppSpacing.s),
                      Text('Route style', style: theme.textTheme.headlineSmall),
                    ],
                  ),
                  const SizedBox(height: AppSpacing.xs),
                  Text(
                    'How the route trades off trails, roads and climbing.',
                    style: theme.textTheme.bodyMedium
                        ?.copyWith(color: theme.colorScheme.onSurfaceVariant),
                  ),
                  const SizedBox(height: AppSpacing.m),
                  for (final p in presets)
                    Padding(
                      padding: const EdgeInsets.only(bottom: AppSpacing.s),
                      child: _PresetTile(
                        preset: p,
                        icon: _presetIcon(p.name),
                        selected: p.name == selected,
                        onTap: () {
                          onSelected(p.name);
                          Navigator.of(sheetCtx).pop();
                        },
                      ),
                    ),
                ],
              ),
            ),
          ),
        );
      },
    );
  }
}

IconData _presetIcon(String name) => switch (name) {
      'balanced' => Icons.balance,
      'avoid_roads' => Icons.forest_outlined,
      'direct' => Icons.straighten,
      'easy_grade' => Icons.trending_down,
      'trail_purist' => Icons.hiking,
      _ => Icons.route_outlined,
    };

/// A rich, selectable preset row: circular icon, bold label, description,
/// with the selected option filled (tonal background + filled icon +
/// check) so the choice reads at a glance.
class _PresetTile extends StatelessWidget {
  final RoutePreset preset;
  final IconData icon;
  final bool selected;
  final VoidCallback onTap;

  const _PresetTile({
    required this.preset,
    required this.icon,
    required this.selected,
    required this.onTap,
  });

  @override
  Widget build(BuildContext context) {
    final scheme = Theme.of(context).colorScheme;
    final text = Theme.of(context).textTheme;
    return Material(
      color: selected ? scheme.secondaryContainer : scheme.surfaceContainerHighest,
      borderRadius: BorderRadius.circular(AppRadius.l),
      clipBehavior: Clip.antiAlias,
      child: InkWell(
        onTap: onTap,
        child: Padding(
          padding: const EdgeInsets.all(AppSpacing.m),
          child: Row(
            children: [
              Container(
                width: 44,
                height: 44,
                decoration: BoxDecoration(
                  color: selected ? scheme.primary : scheme.surface,
                  shape: BoxShape.circle,
                ),
                child: Icon(icon,
                    size: 22,
                    color: selected ? scheme.onPrimary : scheme.onSurfaceVariant),
              ),
              const SizedBox(width: AppSpacing.m),
              Expanded(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  mainAxisSize: MainAxisSize.min,
                  children: [
                    Text(
                      preset.label,
                      style: text.titleMedium?.copyWith(
                        fontWeight: FontWeight.w600,
                        color: selected
                            ? scheme.onSecondaryContainer
                            : scheme.onSurface,
                      ),
                    ),
                    const SizedBox(height: 2),
                    Text(
                      preset.description,
                      style: text.bodySmall?.copyWith(
                        color: selected
                            ? scheme.onSecondaryContainer.withValues(alpha: 0.85)
                            : scheme.onSurfaceVariant,
                      ),
                    ),
                  ],
                ),
              ),
              if (selected) ...[
                const SizedBox(width: AppSpacing.s),
                Icon(Icons.check_circle, color: scheme.primary),
              ],
            ],
          ),
        ),
      ),
    );
  }
}
