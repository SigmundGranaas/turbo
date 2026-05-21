import 'package:flutter/material.dart';
import 'package:flutter_map/flutter_map.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../data/activity_kind_registry.dart';
import '../data/activity_summaries_repository.dart';
import '../models/activity_summary.dart';

/// Renders cross-kind activity summaries on the map. For each summary,
/// looks up the registered kind descriptor and asks it for a marker
/// and/or polyline — composition rather than a hardcoded switch. Falls
/// back to a generic pin if a kind has no `buildMapMarker`. Returns a
/// [Stack] containing first the polyline layer (so routes sit
/// underneath) and then the marker layer (start-pins on top).
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
        point: pos,
        width: 40,
        height: 40,
        child: descriptor?.buildMapMarker != null
            ? descriptor!.buildMapMarker!(s)
            : _DefaultMarker(summary: s, color: _parseHex(s.colorHex)),
      ));
    }

    return Stack(children: [
      if (polylines.isNotEmpty) PolylineLayer(polylines: polylines),
      MarkerLayer(markers: markers),
    ]);
  }

  static Color _parseHex(String? hex) {
    if (hex == null || hex.isEmpty) return Colors.indigo;
    final h = hex.startsWith('#') ? hex.substring(1) : hex;
    final v = int.tryParse(h, radix: 16);
    if (v == null) return Colors.indigo;
    return Color(0xFF000000 | v);
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
