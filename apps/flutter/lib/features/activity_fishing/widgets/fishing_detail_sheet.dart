import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import 'package:turbo/features/activities/api.dart';

import '../data/fishing_repository.dart';
import '../descriptor.dart' show fishingActivityKindDescriptor;
import '../models/fishing_activity.dart';
import '../models/fishing_analysis_extras.dart';
import '../models/fishing_details.dart';
import 'fishing_create_screen.dart';
import 'fishing_observation_extras.dart';

/// Fishing detail sheet — design-aligned chassis.
///
/// Title / stats / description / actions paint instantly from the
/// typed [FishingActivity]; verdict, map overlays, weather metrics,
/// and the solunar bite-window module fill in progressively from
/// [fishingAnalysisProvider]. All four states (loading / ready / error
/// / no-data) are shown without spinner or shimmer — the chassis is the
/// architectural fix for the prior infinite-loading UX.
class FishingDetailSheet extends ConsumerWidget {
  final String activityId;
  const FishingDetailSheet({super.key, required this.activityId});

  static const _kindNoun = 'fishing spot';

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final activityAsync = ref.watch(fishingActivityProvider(activityId));
    return activityAsync.when(
      loading: () => const ActivityLoadingHint(message: 'Loading spot…'),
      error: (e, _) => ActivityLoadingHint(
        icon: Icons.error_outline,
        message: 'Could not load $_kindNoun.',
        subline: '$e',
      ),
      data: (activity) => _Body(activity: activity),
    );
  }
}

class _Body extends ConsumerWidget {
  final FishingActivity activity;
  const _Body({required this.activity});

  static const _tintColor = Color(0xFF1E6FB8);

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final analysisAsync = ref.watch(fishingAnalysisProvider(activity.id));
    final extras = analysisAsync.value == null
        ? null
        : fishingActivityKindDescriptor.parseAnalysisExtras?.call(
            analysisAsync.value!.kindSlices) as FishingAnalysisExtras?;

    return ActivityDetailChassis(
      tintColor: _tintColor,
      icon: Icons.set_meal,
      title: activity.name,
      place: _placeText(activity),
      onClose: () => Navigator.of(context).maybePop(),
      onRefresh: () async {
        ref.invalidate(fishingAnalysisProvider(activity.id));
        try {
          await ref.read(fishingAnalysisProvider(activity.id).future);
        } catch (_) {/* surfaced by the panel */}
      },
      verdict: _verdict(analysisAsync),
      mapPreview: ActivityMapPreviewFromAnalysis(
        points: [activity.position],
        tintColor: _tintColor,
        analysisAsync: analysisAsync,
      ),
      weather: ActivityWeatherPanelFromAnalysis(
        analysisAsync: analysisAsync,
        title: 'Conditions at spot',
        accent: _tintColor,
        summaryBlurb: 'Spot conditions forecast',
        metrics: const [
          WeatherMetrics.wind,
          WeatherMetrics.pressureTrend,
          WeatherMetrics.waterTemp,
        ],
        onRefresh: () => ref.invalidate(fishingAnalysisProvider(activity.id)),
      ),
      stats: ActivityStatStrip(items: _stats(activity.details)),
      module: switch (extras?.biteWindow) {
        final w? when w.end.isAfter(DateTime.now()) => ActivityModuleCard(
            label: 'Solunar window',
            child: Row(
              children: [
                const Icon(Icons.schedule, size: 18, color: _tintColor),
                const SizedBox(width: 8),
                Expanded(
                  child: Text(
                    '${_formatTime(w.start)} – ${_formatTime(w.end)}',
                    style: Theme.of(context).textTheme.bodyMedium?.copyWith(
                        fontWeight: FontWeight.w500),
                  ),
                ),
              ],
            ),
          ),
        _ => null,
      },
      description: activity.description,
      actions: ActivityAction.standardTriple(
        context,
        onLogVisit: () => _logVisit(context, ref),
        onEdit: () => _edit(context),
        onDelete: () => _delete(context, ref),
      ),
    );
  }

  Widget _verdict(AsyncValue<ActivityAnalysis> async) {
    return async.when(
      loading: () => const ActivityVerdict.loading(),
      error: (_, _) => ActivityVerdict.fallback(support: _fallbackHint()),
      data: (a) => ActivityVerdict.fromScore(score: a.score, rationale: a.rationale),
    );
  }

  String _fallbackHint() {
    final d = activity.details;
    return '${_waterLabel(d.waterKind)} · ${_shoreLabel(d.shoreOrBoat)} · refresh to retry';
  }

  static String _placeText(FishingActivity a) {
    final desc = a.description;
    return 'Fishing${desc != null && desc.isNotEmpty ? " · $desc" : ""}';
  }

  static List<StatItem> _stats(FishingDetails d) {
    return [
      StatItem('Water', _waterLabel(d.waterKind)),
      StatItem('Access', _shoreLabel(d.shoreOrBoat)),
      StatItem('Target', _targetLabel(d.targetSpecies)),
    ];
  }

  static String _waterLabel(WaterKind w) => switch (w) {
        WaterKind.river => 'River',
        WaterKind.lake => 'Lake',
        WaterKind.sea => 'Sea',
      };

  static String _shoreLabel(ShoreOrBoat s) => switch (s) {
        ShoreOrBoat.shore => 'Shore',
        ShoreOrBoat.boat => 'Boat',
        ShoreOrBoat.either => 'Either',
      };

  static String _targetLabel(List<TargetSpecies> targets) {
    if (targets.isEmpty) return '—';
    final first = targets.first.speciesCode.trim();
    if (first.isEmpty) return '—';
    final titled = first[0].toUpperCase() + first.substring(1);
    if (targets.length > 1) return '$titled +${targets.length - 1}';
    return titled;
  }

  static String _formatTime(DateTime t) {
    final local = t.toLocal();
    final hh = local.hour.toString().padLeft(2, '0');
    final mm = local.minute.toString().padLeft(2, '0');
    return '$hh:$mm';
  }

  Future<void> _logVisit(BuildContext context, WidgetRef ref) async {
    final saved = await Navigator.of(context).push<bool>(
      MaterialPageRoute(
        builder: (_) => ActivityObservationForm(
          kindKey: 'fishing',
          activityName: activity.name,
          tintColor: _tintColor,
          extrasBuilder: (ctx, draft, onChanged) => [
            FishingObservationExtras(draft: draft, onChanged: onChanged),
          ],
          onSubmit: ({
            required observedAt,
            required rating,
            required comment,
            required photoCount,
            required kindPayload,
          }) async {
            await postObservation(
              ref: ref,
              kindUrlSlug: 'fishing',
              activityId: activity.id,
              observedAt: observedAt,
              rating: rating,
              comment: comment,
              photoCount: photoCount,
              kindPayload: kindPayload,
            );
          },
        ),
      ),
    );
    if (saved == true) {
      ref.invalidate(fishingAnalysisProvider(activity.id));
      if (context.mounted) {
        ScaffoldMessenger.of(context).showSnackBar(
          const SnackBar(content: Text('Visit logged.')),
        );
      }
    }
  }

  void _edit(BuildContext context) {
    Navigator.of(context).pushReplacement(MaterialPageRoute(
      builder: (_) => FishingCreateScreen(
        seedGeometry: ActivityGeometry.fromPoint(activity.position),
        existing: activity,
      ),
    ));
  }

  Future<void> _delete(BuildContext context, WidgetRef ref) async {
    final ok = await showActivityDeleteDialog(
      context,
      name: activity.name,
      kindNoun: 'fishing spot',
    );
    if (!ok) return;
    try {
      await ref.read(fishingRepositoryProvider).delete(activity.id);
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
