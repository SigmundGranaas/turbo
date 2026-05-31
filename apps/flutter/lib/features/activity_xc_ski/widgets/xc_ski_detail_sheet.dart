import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import 'package:turbo/features/activities/api.dart';

import '../data/xc_ski_repository.dart';
import '../descriptor.dart' show xcSkiActivityKindDescriptor;
import '../models/xc_ski_activity.dart';
import '../models/xc_ski_analysis_extras.dart';
import '../models/xc_ski_details.dart';
import 'xc_ski_create_screen.dart';
import 'xc_ski_observation_extras.dart';

/// XC ski detail sheet — design-aligned chassis.
///
/// Title / stats / description / actions paint instantly from the
/// typed [XcSkiActivity]; verdict, map overlays, weather metrics, and
/// the predicted-wax module fill in progressively from
/// [xcSkiAnalysisProvider]. All four states (loading / ready / error /
/// no-data) are shown without spinner or shimmer — the chassis is the
/// architectural fix for the prior infinite-loading UX.
class XcSkiDetailSheet extends ConsumerWidget {
  final String activityId;
  const XcSkiDetailSheet({super.key, required this.activityId});

  static const _kindNoun = 'trail';

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final activityAsync = ref.watch(xcSkiActivityProvider(activityId));
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
  final XcSkiActivity activity;
  const _Body({required this.activity});

  static const _tintColor = Color(0xFF0288D1);

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final analysisAsync = ref.watch(xcSkiAnalysisProvider(activity.id));
    final extras = analysisAsync.value == null
        ? null
        : xcSkiActivityKindDescriptor.parseAnalysisExtras?.call(
            analysisAsync.value!.kindSlices) as XcSkiAnalysisExtras?;

    return ActivityDetailChassis(
      tintColor: _tintColor,
      icon: Icons.nordic_walking,
      title: activity.name,
      place: _placeText(activity),
      onClose: () => Navigator.of(context).maybePop(),
      onRefresh: () async {
        ref.invalidate(xcSkiAnalysisProvider(activity.id));
        // Wait for the new future to settle so the spinner can stop.
        // Errors stay on the analysis async — the weather panel and
        // verdict slots render the error state themselves.
        try {
          await ref.read(xcSkiAnalysisProvider(activity.id).future);
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
        title: 'Track conditions',
        accent: _tintColor,
        summaryBlurb: 'XC track surface forecast',
        metrics: const [
          WeatherMetrics.wind,
          WeatherMetrics.freshSnow24h,
          WeatherMetrics.snowDepth,
        ],
        onRefresh: () => ref.invalidate(xcSkiAnalysisProvider(activity.id)),
      ),
      stats: ActivityStatStrip(items: _stats(activity.details)),
      module: switch (extras?.predictedWax) {
        final wax? => ActivityModuleCard(
            label: 'Predicted wax',
            child: Row(
              children: [
                const Icon(Icons.brush_outlined, size: 18, color: _tintColor),
                const SizedBox(width: 8),
                Expanded(
                  child: Text(
                    wax,
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
    final tag = switch (activity.details.groomingStatus) {
      GroomingStatus.today => 'Stored: groomed today',
      GroomingStatus.yesterday => 'Stored: groomed yesterday',
      GroomingStatus.olderThanTwoDays => 'Stored: groomed >2 days ago',
      GroomingStatus.neverGroomed => 'Backcountry track · never groomed',
      _ => 'Grooming status unknown',
    };
    return '$tag · refresh to retry';
  }

  static String _placeText(XcSkiActivity a) {
    final desc = a.description;
    return 'XC Ski${desc != null && desc.isNotEmpty ? " · $desc" : ""}';
  }

  static List<StatItem> _stats(XcSkiDetails d) {
    final technique = switch (d.technique) {
      XcSkiTechnique.classic => 'Classic',
      XcSkiTechnique.skate => 'Skate',
      XcSkiTechnique.both => 'Both',
      XcSkiTechnique.backcountry => 'Backcountry',
    };
    return [
      StatItem('Distance', _distance(d.distanceMeters)),
      StatItem('Ascent', '${d.ascentMeters} m'),
      StatItem('Technique', technique),
    ];
  }

  static String _distance(int meters) =>
      meters >= 1000 ? '${(meters / 1000).toStringAsFixed(1)} km' : '$meters m';

  Future<void> _logVisit(BuildContext context, WidgetRef ref) async {
    final saved = await Navigator.of(context).push<bool>(
      MaterialPageRoute(
        builder: (_) => ActivityObservationForm(
          kindKey: 'xc_ski',
          activityName: activity.name,
          tintColor: _tintColor,
          extrasBuilder: (ctx, draft, onChanged) => [
            XcSkiObservationExtras(draft: draft, onChanged: onChanged),
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
              kindUrlSlug: 'xc-ski',
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
      ref.invalidate(xcSkiAnalysisProvider(activity.id));
      if (context.mounted) {
        ScaffoldMessenger.of(context).showSnackBar(
          const SnackBar(content: Text('Visit logged.')),
        );
      }
    }
  }

  void _edit(BuildContext context) {
    Navigator.of(context).pushReplacement(MaterialPageRoute(
      builder: (_) => XcSkiCreateScreen(
        seedGeometry: ActivityGeometry.fromRoute(activity.route),
        existing: activity,
      ),
    ));
  }

  Future<void> _delete(BuildContext context, WidgetRef ref) async {
    final ok = await showActivityDeleteDialog(
      context,
      name: activity.name,
      kindNoun: 'trail',
    );
    if (!ok) return;
    try {
      await ref.read(xcSkiRepositoryProvider).delete(activity.id);
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
