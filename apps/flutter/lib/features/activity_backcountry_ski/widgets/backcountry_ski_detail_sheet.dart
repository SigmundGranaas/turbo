import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import 'package:turbo/features/activities/api.dart';

import '../data/backcountry_ski_repository.dart';
import '../descriptor.dart' show backcountrySkiActivityKindDescriptor;
import '../models/backcountry_ski_activity.dart';
import '../models/backcountry_ski_analysis_extras.dart';
import '../models/backcountry_ski_details.dart';
import 'backcountry_ski_create_screen.dart';
import 'backcountry_ski_observation_extras.dart';

/// Backcountry-ski detail sheet — design-aligned chassis.
///
/// Title / stats / description / actions paint instantly from the
/// typed [BackcountrySkiActivity]; verdict, map overlays, weather
/// metrics, and the per-aspect loading module fill in progressively
/// from [backcountrySkiAnalysisProvider]. All four states
/// (loading / ready / error / no-data) are shown without spinner or
/// shimmer — the chassis is the architectural fix for the prior
/// infinite-loading UX.
class BackcountrySkiDetailSheet extends ConsumerWidget {
  final String activityId;
  const BackcountrySkiDetailSheet({super.key, required this.activityId});

  static const _kindNoun = 'tour';

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final activityAsync = ref.watch(backcountrySkiActivityProvider(activityId));
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
  final BackcountrySkiActivity activity;
  const _Body({required this.activity});

  static const _tintColor = Color(0xFF5E72A5);

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final analysisAsync =
        ref.watch(backcountrySkiAnalysisProvider(activity.id));
    final extras = analysisAsync.value == null
        ? null
        : backcountrySkiActivityKindDescriptor.parseAnalysisExtras?.call(
            analysisAsync.value!.kindSlices) as BackcountrySkiAnalysisExtras?;

    return ActivityDetailChassis(
      tintColor: _tintColor,
      icon: Icons.downhill_skiing,
      title: activity.name,
      place: _placeText(activity),
      onClose: () => Navigator.of(context).maybePop(),
      onRefresh: () async {
        ref.invalidate(backcountrySkiAnalysisProvider(activity.id));
        try {
          await ref.read(backcountrySkiAnalysisProvider(activity.id).future);
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
        title: 'Route conditions',
        accent: _tintColor,
        summaryBlurb: 'Route surface forecast',
        metrics: const [
          WeatherMetrics.wind,
          WeatherMetrics.freshSnow24h,
          WeatherMetrics.snowDepth,
        ],
        onRefresh: () =>
            ref.invalidate(backcountrySkiAnalysisProvider(activity.id)),
      ),
      stats: ActivityStatStrip(items: _stats(activity.details)),
      module: switch (extras?.perAspect) {
        final list? when list.isNotEmpty => ActivityModuleCard(
            label: 'Aspect loading',
            child: Wrap(
              spacing: 6,
              runSpacing: 6,
              children: list
                  .map((a) => _AspectChip(loading: a))
                  .toList(growable: false),
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
      data: (a) =>
          ActivityVerdict.fromScore(score: a.score, rationale: a.rationale),
    );
  }

  String _fallbackHint() =>
      'ATES ${_atesLabel(activity.details.atesRating)} · refresh to retry';

  static String _placeText(BackcountrySkiActivity a) {
    final desc = a.description;
    return 'Backcountry ski${desc != null && desc.isNotEmpty ? " · $desc" : ""}';
  }

  static String _atesLabel(AtesRating? r) {
    return switch (r) {
      AtesRating.simple => 'Simple',
      AtesRating.challenging => 'Challenging',
      AtesRating.complex => 'Complex',
      _ => '—',
    };
  }

  static List<StatItem> _stats(BackcountrySkiDetails d) {
    return [
      StatItem('Distance', _distance(d.distanceMeters)),
      StatItem('Ascent', '${d.ascentMeters} m'),
      StatItem('ATES', _atesLabel(d.atesRating)),
    ];
  }

  static String _distance(int meters) =>
      meters >= 1000 ? '${(meters / 1000).toStringAsFixed(1)} km' : '$meters m';

  Future<void> _logVisit(BuildContext context, WidgetRef ref) async {
    final saved = await Navigator.of(context).push<bool>(
      MaterialPageRoute(
        builder: (_) => ActivityObservationForm(
          kindKey: 'backcountry_ski',
          activityName: activity.name,
          tintColor: _tintColor,
          extrasBuilder: (ctx, draft, onChanged) => [
            BackcountrySkiObservationExtras(draft: draft, onChanged: onChanged),
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
              kindUrlSlug: 'backcountry-ski',
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
      ref.invalidate(backcountrySkiAnalysisProvider(activity.id));
      if (context.mounted) {
        ScaffoldMessenger.of(context).showSnackBar(
          const SnackBar(content: Text('Visit logged.')),
        );
      }
    }
  }

  void _edit(BuildContext context) {
    Navigator.of(context).pushReplacement(MaterialPageRoute(
      builder: (_) => BackcountrySkiCreateScreen(
        seedGeometry: ActivityGeometry.fromRoute(activity.route),
        existing: activity,
      ),
    ));
  }

  Future<void> _delete(BuildContext context, WidgetRef ref) async {
    final ok = await showActivityDeleteDialog(
      context,
      name: activity.name,
      kindNoun: 'tour',
    );
    if (!ok) return;
    try {
      await ref.read(backcountrySkiRepositoryProvider).delete(activity.id);
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

class _AspectChip extends StatelessWidget {
  final AspectLoading loading;
  const _AspectChip({required this.loading});

  @override
  Widget build(BuildContext context) {
    final t = loading.loadedFractionOfFraction.clamp(0.0, 1.0);
    final bg = Color.lerp(ConditionPalette.good, ConditionPalette.stop, t) ??
        ConditionPalette.good;
    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 4),
      decoration: BoxDecoration(
        color: bg,
        borderRadius: BorderRadius.circular(10),
      ),
      child: Text(
        loading.aspect.toUpperCase(),
        style: const TextStyle(
          color: Colors.white,
          fontSize: 12,
          fontWeight: FontWeight.w600,
        ),
      ),
    );
  }
}
