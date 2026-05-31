import 'package:flutter/material.dart';
import 'package:flutter_map/flutter_map.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../data/activity_kind_registry.dart';
import '../data/activity_summaries_repository.dart';
import '../models/activity_kind_descriptor.dart';
import '../models/activity_summary.dart';
import 'activity_detail_screen.dart';

/// Renders cross-kind activity summaries on the map. For each summary,
/// looks up the registered kind descriptor and asks it for a marker
/// and/or polyline — composition rather than a hardcoded switch. Falls
/// back to a generic pin if a kind has no `buildMapMarker`. Returns a
/// [Stack] containing first the polyline layer (so routes sit
/// underneath) and then the marker layer (start-pins on top).
///
/// Tap behaviour: a kind that has opted into the new shell by setting
/// [ActivityKindDescriptor.buildDetailContent] pushes a full-screen
/// [ActivityDetailScreen] route. Kinds that haven't migrated fall back
/// to a modal bottom sheet on [buildDetailScreen].
///
/// Each pin also renders a score halo (green/amber/red) when the
/// summary carries a fresh [ActivitySummary.summaryScore]. The halo
/// lets the map itself act as a recommender without per-pin analysis
/// fetches.
class ActivitiesMapLayer extends ConsumerWidget {
  const ActivitiesMapLayer({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final summariesAsync = ref.watch(activitySummariesRepositoryProvider);
    final registry = ref.watch(activityKindRegistryProvider);
    final summaries = summariesAsync.value ?? const {};

    final markers = <Marker>[];
    final polylines = <Polyline>[];

    for (final s in summaries.values) {
      final descriptor = registry.get(s.kind);

      // Polyline first — only if the kind opts in. Point kinds skip.
      final polyline = descriptor?.buildMapPolyline?.call(s);
      if (polyline != null) polylines.add(polyline);

      // Marker — fall back to a generic pin if the kind didn't supply
      // one but did produce coordinates we can pin (e.g. polyline start).
      final pos = s.geometry.firstPoint;
      if (pos == null) continue;
      markers.add(Marker(
        // Stable per-activity key so integration tests can target the
        // exact pin (`Key('activity-pin-<id>')`). Cheap at runtime.
        key: Key('activity-pin-${s.id}'),
        point: pos,
        width: 44,
        height: 44,
        child: GestureDetector(
          behavior: HitTestBehavior.opaque,
          onTap: () => _openDetail(context, descriptor, s.id),
          child: _ScoreHalo(
            score: _freshScore(s),
            child: descriptor?.buildMapMarker != null
                ? descriptor!.buildMapMarker!(s)
                : _DefaultMarker(summary: s, color: _parseHex(s.colorHex)),
          ),
        ),
      ));
    }

    return Stack(children: [
      if (polylines.isNotEmpty) PolylineLayer(polylines: polylines),
      MarkerLayer(markers: markers),
    ]);
  }

  void _openDetail(BuildContext context, ActivityKindDescriptor? descriptor, String id) {
    if (descriptor == null) return;
    if (descriptor.buildDetailContent != null) {
      Navigator.of(context).push(MaterialPageRoute<void>(
        builder: (_) => ActivityDetailScreen(descriptor: descriptor, activityId: id),
      ));
      return;
    }
    showModalBottomSheet<void>(
      context: context,
      isScrollControlled: true,
      useSafeArea: true,
      builder: (ctx) => descriptor.buildDetailScreen(ctx, id),
    );
  }

  /// Discard score halos older than 3h — stale data isn't honest
  /// recommendation data; better to render the kind tint without a
  /// halo than to lie.
  static int? _freshScore(ActivitySummary s) {
    if (s.summaryScore == null) return null;
    final at = s.summaryScoreAt;
    if (at == null) return s.summaryScore;
    if (DateTime.now().toUtc().difference(at.toUtc()) > const Duration(hours: 3)) return null;
    return s.summaryScore;
  }

  static Color _parseHex(String? hex) {
    if (hex == null || hex.isEmpty) return Colors.indigo;
    final h = hex.startsWith('#') ? hex.substring(1) : hex;
    final v = int.tryParse(h, radix: 16);
    if (v == null) return Colors.indigo;
    return Color(0xFF000000 | v);
  }
}

/// Adds a score-tinted ring around a kind's marker so the map shades
/// pins by today's analysis without each kind needing to re-implement
/// it. Returns the child unchanged when [score] is null.
class _ScoreHalo extends StatelessWidget {
  final int? score;
  final Widget child;
  const _ScoreHalo({required this.score, required this.child});

  @override
  Widget build(BuildContext context) {
    if (score == null) return Center(child: child);
    final color = score! >= 70
        ? Colors.green.shade600
        : score! >= 40
            ? Colors.amber.shade700
            : Colors.red.shade600;
    return Center(
      child: Container(
        padding: const EdgeInsets.all(2),
        decoration: BoxDecoration(
          shape: BoxShape.circle,
          border: Border.all(color: color, width: 2.5),
          color: color.withValues(alpha: 0.10),
        ),
        child: child,
      ),
    );
  }
}

class _DefaultMarker extends StatelessWidget {
  final ActivitySummary summary;
  final Color color;
  const _DefaultMarker({required this.summary, required this.color});

  @override
  Widget build(BuildContext context) {
    return Icon(Icons.place, color: color, size: 32, semanticLabel: summary.name);
  }
}
