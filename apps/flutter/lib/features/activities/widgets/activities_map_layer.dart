import 'package:flutter/material.dart';
import 'package:turbo/core/widgets/exclusive_sheet.dart';
import 'package:turbo/core/widgets/map/map_marker_pin.dart';
import 'package:flutter_map/flutter_map.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../data/activity_kind_registry.dart';
import '../data/activity_summaries_repository.dart';
import '../models/activity_kind_descriptor.dart';
import '../models/activity_summary.dart';
import 'activity_detail_screen.dart';

/// Renders cross-kind activity summaries on the map. Each summary is pinned
/// with the shared [MapMarkerPin] carrying the kind's icon (one marker type
/// across the whole app), and the kind's optional [buildMapPolyline] draws the
/// route underneath — composition rather than a hardcoded switch.
///
/// Tap behaviour: a kind that has opted into the new shell by setting
/// [ActivityKindDescriptor.buildDetailContent] pushes a full-screen
/// [ActivityDetailScreen] route. Kinds that haven't migrated fall back
/// to a modal bottom sheet on [buildDetailScreen].
///
/// When a summary carries a fresh [ActivitySummary.summaryScore], the pin's
/// glyph is tinted green/amber/red so the map itself acts as a recommender
/// without per-pin analysis fetches.
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
      // Same pin as every other map entity (MapMarkerPin), carrying the kind's
      // icon. The score still shades the pin — now as the glyph tint (fresh
      // green/amber/red) rather than a bespoke ring — so activities read as the
      // same marker type as markers / saved paths instead of each kind drawing
      // its own widget.
      final score = _freshScore(s);
      final accent = score != null
          ? _scoreColor(score)
          : (descriptor?.tintColor ?? _parseHex(s.colorHex));
      markers.add(Marker(
        // Stable per-activity key so integration tests can target the
        // exact pin (`Key('activity-pin-<id>')`). Cheap at runtime.
        key: Key('activity-pin-${s.id}'),
        point: pos,
        width: MapMarkerPin.baseWidth,
        height: MapMarkerPin.baseHeight,
        alignment: Alignment.bottomCenter,
        child: MapMarkerPin(
          icon: descriptor?.icon ?? Icons.place,
          accent: accent,
          title: s.name,
          onTap: () => _openDetail(context, descriptor, s.id),
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
    showExclusiveSheet<void>(
      context,
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

  /// Fresh-score → pin tint (the recommender shading), green/amber/red.
  static Color _scoreColor(int score) => score >= 70
      ? Colors.green.shade600
      : score >= 40
          ? Colors.amber.shade700
          : Colors.red.shade600;
}
