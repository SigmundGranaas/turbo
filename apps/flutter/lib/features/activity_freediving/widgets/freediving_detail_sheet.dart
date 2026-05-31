import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import 'package:turbo/features/activities/api.dart';

import '../data/freediving_repository.dart';
import '../descriptor.dart' show freedivingActivityKindDescriptor;
import '../models/freediving_activity.dart';
import '../models/freediving_analysis_extras.dart';
import '../models/freediving_details.dart';
import 'freediving_create_screen.dart';
import 'freediving_observation_extras.dart';

/// Freediving detail sheet — design-aligned chassis.
///
/// Title / stats / description / actions paint instantly from the
/// typed [FreedivingActivity]; verdict, map overlays, weather metrics,
/// and the visibility/tide module fill in progressively from
/// [freedivingAnalysisProvider]. All four states (loading / ready /
/// error / no-data) are shown without spinner or shimmer — the
/// chassis is the architectural fix for the prior infinite-loading
/// UX.
class FreedivingDetailSheet extends ConsumerWidget {
  final String activityId;
  const FreedivingDetailSheet({super.key, required this.activityId});

  static const _kindNoun = 'dive site';

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final activityAsync = ref.watch(freedivingActivityProvider(activityId));
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
  final FreedivingActivity activity;
  const _Body({required this.activity});

  static const _tintColor = Color(0xFF1565C0);

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final analysisAsync = ref.watch(freedivingAnalysisProvider(activity.id));
    final extras = analysisAsync.value == null
        ? null
        : freedivingActivityKindDescriptor.parseAnalysisExtras?.call(
            analysisAsync.value!.kindSlices) as FreedivingAnalysisExtras?;

    return ActivityDetailChassis(
      tintColor: _tintColor,
      icon: Icons.scuba_diving,
      title: activity.name,
      place: _placeText(activity),
      onClose: () => Navigator.of(context).maybePop(),
      onRefresh: () async {
        ref.invalidate(freedivingAnalysisProvider(activity.id));
        try {
          await ref.read(freedivingAnalysisProvider(activity.id).future);
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
        summaryBlurb: 'Dive site forecast',
        metrics: const [
          WeatherMetrics.wind,
          WeatherMetrics.seaTemp,
          WeatherMetrics.waveHeight,
          WeatherMetrics.vizMeters,
        ],
        onRefresh: () => ref.invalidate(freedivingAnalysisProvider(activity.id)),
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
      data: (a) => ActivityVerdict.fromScore(score: a.score, rationale: a.rationale),
    );
  }

  String _fallbackHint() =>
      '${_waterBodyLabel(activity.details.waterBody)} dive site · refresh to retry';

  ActivityModuleCard? _module(BuildContext context, FreedivingAnalysisExtras? extras) {
    return switch (extras) {
      FreedivingAnalysisExtras(vizForecast: final viz?) => ActivityModuleCard(
          label: 'Visibility forecast',
          child: _moduleRow(
            context,
            icon: Icons.visibility,
            text: _vizText(viz),
          ),
        ),
      FreedivingAnalysisExtras(tide: final tide?) => ActivityModuleCard(
          label: 'Tide',
          child: _moduleRow(
            context,
            icon: Icons.water,
            text: _tideText(tide),
          ),
        ),
      _ => null,
    };
  }

  static Widget _moduleRow(BuildContext context,
      {required IconData icon, required String text}) {
    return Row(
      children: [
        Icon(icon, size: 18, color: _tintColor),
        const SizedBox(width: 8),
        Expanded(
          child: Text(
            text,
            style: Theme.of(context)
                .textTheme
                .bodyMedium
                ?.copyWith(fontWeight: FontWeight.w500),
          ),
        ),
      ],
    );
  }

  static String _vizText(VizForecast viz) {
    final dir = (viz.direction?.isNotEmpty ?? false) ? ' · ${viz.direction}' : '';
    final range = viz.low == viz.high
        ? '${viz.high.toStringAsFixed(0)} m'
        : '${viz.low.toStringAsFixed(0)}–${viz.high.toStringAsFixed(0)} m';
    return '$range$dir';
  }

  static String _tideText(TideInfo tide) {
    final parts = <String>[
      if (tide.summary != null && tide.summary!.isNotEmpty) tide.summary!,
      if (tide.heightM != null) '${tide.heightM!.toStringAsFixed(2)} m',
    ];
    return parts.isEmpty ? 'Next slack —' : parts.join(' · ');
  }

  static String _placeText(FreedivingActivity a) {
    final desc = a.description;
    return 'Freediving${desc != null && desc.isNotEmpty ? " · $desc" : ""}';
  }

  static List<StatItem> _stats(FreedivingDetails d) => [
        StatItem('Water', _waterBodyLabel(d.waterBody)),
        StatItem('Bottom', _bottomLabel(d.bottomType)),
        StatItem('Max depth', '${d.maxDepthMeters.toStringAsFixed(0)} m'),
      ];

  static String _waterBodyLabel(WaterBody w) => switch (w) {
        WaterBody.sea => 'Sea',
        WaterBody.fjord => 'Fjord',
        WaterBody.lake => 'Lake',
      };

  static String _bottomLabel(BottomType b) => switch (b) {
        BottomType.unknown => 'Unknown',
        BottomType.sandyShallow => 'Sandy shallow',
        BottomType.rockyShallow => 'Rocky shallow',
        BottomType.kelpForest => 'Kelp forest',
        BottomType.wall => 'Wall',
        BottomType.reef => 'Reef',
        BottomType.seagrassMeadow => 'Seagrass',
        BottomType.open => 'Open',
      };

  Future<void> _logVisit(BuildContext context, WidgetRef ref) async {
    final saved = await Navigator.of(context).push<bool>(
      MaterialPageRoute(
        builder: (_) => ActivityObservationForm(
          kindKey: 'freediving',
          activityName: activity.name,
          tintColor: _tintColor,
          extrasBuilder: (ctx, draft, onChanged) => [
            FreedivingObservationExtras(draft: draft, onChanged: onChanged),
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
              kindUrlSlug: 'freediving',
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
      ref.invalidate(freedivingAnalysisProvider(activity.id));
      if (context.mounted) {
        ScaffoldMessenger.of(context).showSnackBar(
          const SnackBar(content: Text('Visit logged.')),
        );
      }
    }
  }

  void _edit(BuildContext context) {
    Navigator.of(context).pushReplacement(MaterialPageRoute(
      builder: (_) => FreedivingCreateScreen(
        seedGeometry: ActivityGeometry.fromPoint(activity.position),
        existing: activity,
      ),
    ));
  }

  Future<void> _delete(BuildContext context, WidgetRef ref) async {
    final ok = await showActivityDeleteDialog(
      context,
      name: activity.name,
      kindNoun: 'dive site',
    );
    if (!ok) return;
    try {
      await ref.read(freedivingRepositoryProvider).delete(activity.id);
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
