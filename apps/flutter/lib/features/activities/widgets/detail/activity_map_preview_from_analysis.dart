import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:latlong2/latlong.dart';

import '../../models/activity_analysis.dart';
import '../../models/driver_keys.dart';
import '../activity_conditions_map.dart';

/// Map thumbnail with weather overlays sourced from the analysis. The
/// underlying [ActivityConditionsMap] paints the geometry the moment
/// it's available; the temp/wind overlays appear progressively as the
/// analysis resolves. Nothing here is gated on the analysis call —
/// the map itself is always shown.
///
/// This is the shared replacement for the per-kind `_MapPreview`
/// helper class that used to live in each detail sheet.
class ActivityMapPreviewFromAnalysis extends StatelessWidget {
  final List<LatLng> points;
  final Color tintColor;
  final AsyncValue<ActivityAnalysis> analysisAsync;
  final double height;

  const ActivityMapPreviewFromAnalysis({
    super.key,
    required this.points,
    required this.tintColor,
    required this.analysisAsync,
    this.height = 160,
  });

  @override
  Widget build(BuildContext context) {
    final analysis = analysisAsync.value;
    return Padding(
      padding: const EdgeInsets.only(top: 12),
      child: ActivityConditionsMap(
        points: points,
        tintColor: tintColor,
        airTemperatureCelsius:
            analysis == null ? null : _driver(analysis, DriverKeys.tempBand),
        windSpeedMs:
            analysis == null ? null : _driver(analysis, DriverKeys.wind),
        height: height,
      ),
    );
  }

  static double? _driver(ActivityAnalysis a, String key) {
    for (final d in a.drivers) {
      if (d.key == key) return d.value;
    }
    return null;
  }
}
