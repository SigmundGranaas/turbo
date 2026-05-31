import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import 'package:turbo/features/activities/api.dart';

import '../data/hiking_repository.dart';
import '../models/hiking_activity.dart';
import '../models/hiking_details.dart';
import 'hiking_create_screen.dart';
import 'hiking_observation_extras.dart';

/// Hiking detail sheet — design-aligned chassis.
///
/// Title / stats / description / actions paint instantly from the
/// typed [HikingActivity]; verdict, map overlays, and weather metrics
/// fill in progressively from [hikingAnalysisProvider]. All four
/// states (loading / ready / error / no-data) are shown without
/// spinner or shimmer — the chassis is the architectural fix for the
/// prior infinite-loading UX.
class HikingDetailSheet extends ConsumerWidget {
  final String activityId;
  const HikingDetailSheet({super.key, required this.activityId});

  static const _kindNoun = 'hike';

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final activityAsync = ref.watch(hikingActivityProvider(activityId));
    return activityAsync.when(
      loading: () => const ActivityLoadingHint(message: 'Loading $_kindNoun…'),
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
  final HikingActivity activity;
  const _Body({required this.activity});

  static const _tintColor = Color(0xFF2E7D32);

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final analysisAsync = ref.watch(hikingAnalysisProvider(activity.id));

    return ActivityDetailChassis(
      tintColor: _tintColor,
      icon: Icons.hiking,
      title: activity.name,
      place: _placeText(activity),
      onClose: () => Navigator.of(context).maybePop(),
      onRefresh: () async {
        ref.invalidate(hikingAnalysisProvider(activity.id));
        // Wait for the new future to settle so the spinner can stop.
        // Errors stay on the analysis async — the weather panel and
        // verdict slots render the error state themselves.
        try {
          await ref.read(hikingAnalysisProvider(activity.id).future);
        } catch (_) {/* surfaced by the panel */}
      },
      verdict: _verdict(analysisAsync),
      mapPreview: ActivityMapPreviewFromAnalysis(
        points: activity.route,
        tintColor: _tintColor,
        analysisAsync: analysisAsync,
      ),
      weather: ActivityWeatherPanelFromAnalysis(
        analysisAsync: analysisAsync,
        title: 'Trail conditions',
        accent: _tintColor,
        summaryBlurb: 'Trail weather forecast',
        metrics: const [
          WeatherMetrics.wind,
          WeatherMetrics.rain24h,
        ],
        onRefresh: () => ref.invalidate(hikingAnalysisProvider(activity.id)),
      ),
      stats: ActivityStatStrip(items: _stats(activity.details)),
      // no kind module — hiking extras carry only DEM geometry numbers
      module: null,
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
    return 'Stored: ${_distance(activity.details.distanceMeters)} trail · refresh to retry';
  }

  static String _placeText(HikingActivity a) {
    final desc = a.description;
    return 'Hike${desc != null && desc.isNotEmpty ? " · $desc" : ""}';
  }

  static List<StatItem> _stats(HikingDetails d) {
    return [
      StatItem('Distance', _distance(d.distanceMeters)),
      StatItem('Ascent', '${d.ascentMeters} m'),
      StatItem('Difficulty', _difficultyLabel(d.difficulty)),
    ];
  }

  static String _difficultyLabel(HikingDifficulty d) => switch (d) {
        HikingDifficulty.easy => 'Easy',
        HikingDifficulty.moderate => 'Moderate',
        HikingDifficulty.hard => 'Hard',
        HikingDifficulty.expert => 'Expert',
      };

  static String _distance(int meters) =>
      meters >= 1000 ? '${(meters / 1000).toStringAsFixed(1)} km' : '$meters m';

  Future<void> _logVisit(BuildContext context, WidgetRef ref) async {
    final saved = await Navigator.of(context).push<bool>(
      MaterialPageRoute(
        builder: (_) => ActivityObservationForm(
          kindKey: 'hiking',
          activityName: activity.name,
          tintColor: _tintColor,
          extrasBuilder: (ctx, draft, onChanged) => [
            HikingObservationExtras(draft: draft, onChanged: onChanged),
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
              kindUrlSlug: 'hiking',
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
      ref.invalidate(hikingAnalysisProvider(activity.id));
      if (context.mounted) {
        ScaffoldMessenger.of(context).showSnackBar(
          const SnackBar(content: Text('Visit logged.')),
        );
      }
    }
  }

  void _edit(BuildContext context) {
    Navigator.of(context).pushReplacement(MaterialPageRoute(
      builder: (_) => HikingCreateScreen(
        seedGeometry: ActivityGeometry.fromRoute(activity.route),
        existing: activity,
      ),
    ));
  }

  Future<void> _delete(BuildContext context, WidgetRef ref) async {
    final ok = await showActivityDeleteDialog(
      context,
      name: activity.name,
      kindNoun: 'hike',
    );
    if (!ok) return;
    try {
      await ref.read(hikingRepositoryProvider).delete(activity.id);
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
