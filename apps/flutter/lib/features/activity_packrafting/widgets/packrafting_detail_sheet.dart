import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import 'package:turbo/features/activities/api.dart';

import '../data/packrafting_repository.dart';
import '../descriptor.dart' show packraftingActivityKindDescriptor;
import '../models/packrafting_activity.dart';
import '../models/packrafting_analysis_extras.dart';
import '../models/packrafting_details.dart';
import 'packrafting_create_screen.dart';
import 'packrafting_observation_extras.dart';

/// Packrafting detail sheet — design-aligned chassis.
///
/// Title / stats / description / actions paint instantly from the
/// typed [PackraftingActivity]; verdict, map overlays, weather metrics,
/// and the flow-vs-season module fill in progressively from
/// [packraftingAnalysisProvider]. All four states (loading / ready /
/// error / no-data) are shown without spinner or shimmer — the chassis
/// is the architectural fix for the prior infinite-loading UX.
class PackraftingDetailSheet extends ConsumerWidget {
  final String activityId;
  const PackraftingDetailSheet({super.key, required this.activityId});

  static const _kindNoun = 'float';

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final activityAsync = ref.watch(packraftingActivityProvider(activityId));
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
  final PackraftingActivity activity;
  const _Body({required this.activity});

  static const _tintColor = Color(0xFF00838F);

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final analysisAsync = ref.watch(packraftingAnalysisProvider(activity.id));
    final extras = analysisAsync.value == null
        ? null
        : packraftingActivityKindDescriptor.parseAnalysisExtras?.call(
            analysisAsync.value!.kindSlices) as PackraftingAnalysisExtras?;

    return ActivityDetailChassis(
      tintColor: _tintColor,
      icon: Icons.kayaking,
      title: activity.name,
      place: _placeText(activity),
      onClose: () => Navigator.of(context).maybePop(),
      onRefresh: () async {
        ref.invalidate(packraftingAnalysisProvider(activity.id));
        try {
          await ref.read(packraftingAnalysisProvider(activity.id).future);
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
        summaryBlurb: 'River conditions forecast',
        metrics: const [
          WeatherMetrics.wind,
          WeatherMetrics.flowCumecs,
          WeatherMetrics.rain24h,
        ],
        onRefresh: () =>
            ref.invalidate(packraftingAnalysisProvider(activity.id)),
      ),
      stats: ActivityStatStrip(items: _stats(activity.details)),
      module: _module(context, extras),
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
      'Grade ${_gradeLabel(activity.details.maxGrade)} · refresh to retry';

  ActivityModuleCard? _module(
      BuildContext context, PackraftingAnalysisExtras? extras) {
    final pct = extras?.percentile;
    if (pct == null) return null;
    final pctInt = (pct * 100).round().clamp(0, 100);
    final flow = extras?.currentCumecs;
    final flowStr = flow == null ? '' : ' · ${flow.toStringAsFixed(1)} m³/s';
    return ActivityModuleCard(
      label: 'Today vs season',
      child: Row(
        children: [
          const Icon(Icons.water_drop, size: 18, color: _tintColor),
          const SizedBox(width: 8),
          Expanded(
            child: Text(
              '${_ordinal(pctInt)} percentile$flowStr',
              style: Theme.of(context)
                  .textTheme
                  .bodyMedium
                  ?.copyWith(fontWeight: FontWeight.w500),
            ),
          ),
        ],
      ),
    );
  }

  static String _ordinal(int n) {
    if (n % 100 >= 11 && n % 100 <= 13) return '${n}th';
    return switch (n % 10) {
      1 => '${n}st',
      2 => '${n}nd',
      3 => '${n}rd',
      _ => '${n}th',
    };
  }

  static String _placeText(PackraftingActivity a) {
    final desc = a.description;
    return 'Packrafting${desc != null && desc.isNotEmpty ? " · $desc" : ""}';
  }

  static List<StatItem> _stats(PackraftingDetails d) {
    final portages = _countPortages(d.segments);
    return [
      StatItem('Distance', _distance(d.distanceMeters)),
      StatItem('Grade', _gradeLabel(d.maxGrade)),
      StatItem('Portages', portages == null ? '—' : '$portages'),
    ];
  }

  static int? _countPortages(List<RouteSegment> segments) {
    if (segments.isEmpty) return null;
    return segments.where((s) => s.kind == SegmentKind.portage).length;
  }

  static String _gradeLabel(WaterGrade g) => switch (g) {
        WaterGrade.flatwater => 'Flatwater',
        WaterGrade.i => 'I',
        WaterGrade.ii => 'II',
        WaterGrade.iii => 'III',
        WaterGrade.iv => 'IV',
        WaterGrade.v => 'V',
        WaterGrade.vi => 'VI',
      };

  static String _distance(int meters) =>
      meters >= 1000 ? '${(meters / 1000).toStringAsFixed(1)} km' : '$meters m';

  Future<void> _logVisit(BuildContext context, WidgetRef ref) async {
    final saved = await Navigator.of(context).push<bool>(
      MaterialPageRoute(
        builder: (_) => ActivityObservationForm(
          kindKey: 'packrafting',
          activityName: activity.name,
          tintColor: _tintColor,
          extrasBuilder: (ctx, draft, onChanged) => [
            PackraftingObservationExtras(draft: draft, onChanged: onChanged),
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
              kindUrlSlug: 'packrafting',
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
      ref.invalidate(packraftingAnalysisProvider(activity.id));
      if (context.mounted) {
        ScaffoldMessenger.of(context).showSnackBar(
          const SnackBar(content: Text('Visit logged.')),
        );
      }
    }
  }

  void _edit(BuildContext context) {
    Navigator.of(context).pushReplacement(MaterialPageRoute(
      builder: (_) => PackraftingCreateScreen(
        seedGeometry: ActivityGeometry.fromRoute(activity.route),
        existing: activity,
      ),
    ));
  }

  Future<void> _delete(BuildContext context, WidgetRef ref) async {
    final ok = await showActivityDeleteDialog(
      context,
      name: activity.name,
      kindNoun: 'float',
    );
    if (!ok) return;
    try {
      await ref.read(packraftingRepositoryProvider).delete(activity.id);
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
