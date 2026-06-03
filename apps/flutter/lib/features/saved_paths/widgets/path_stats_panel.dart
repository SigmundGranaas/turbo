import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:latlong2/latlong.dart';

import 'package:turbo/app/tokens.dart';
import 'package:turbo/core/geo/geo_metrics.dart';
import 'package:turbo/core/geo/geo_path.dart';
import 'package:turbo/features/settings/api.dart';

import 'elevation_profile.dart';

/// One stats + elevation-profile panel for *any* [GeoPath] — saved path,
/// curated trail, planned route, recorded track or activity route. Distance /
/// ascent / descent / estimated time come from the path or the shared
/// [GeoMetrics]; the profile reuses [ElevationProfile]. Tapping any line now
/// shows the same summary instead of each feature rolling its own. (Tier 3 of
/// the cohesion pass.)
class PathStatsPanel extends ConsumerWidget {
  final GeoPath path;
  final bool showProfile;

  /// The planned route this track was recorded against, if any. When present a
  /// "followed N% · max M off" row summarises how closely the actual [path]
  /// matched the plan (via [GeoMetrics.deviation]).
  final List<LatLng>? plannedGeometry;

  const PathStatsPanel({
    super.key,
    required this.path,
    this.showProfile = true,
    this.plannedGeometry,
  });

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final unit = ref.watch(settingsProvider
        .select((s) => s.value?.distanceUnit ?? DistanceUnit.metric));
    final scheme = Theme.of(context).colorScheme;

    final elev = path.elevations;
    double? ascent = path.ascentM;
    double? descent = path.descentM;
    if (ascent == null && elev != null) {
      final ad = GeoMetrics.ascentDescent(elev);
      ascent = ad.ascent;
      descent = ad.descent;
    }
    final etaSeconds =
        GeoMetrics.naismithSeconds(path.distanceM, ascentM: ascent ?? 0);

    final hasProfile = showProfile &&
        elev != null &&
        elev.where((e) => e != null && !e.isNaN).length >= 2;

    final planned = plannedGeometry;
    final deviation = (planned != null && planned.length >= 2)
        ? GeoMetrics.deviation(path.points, planned)
        : null;

    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Wrap(
          spacing: AppSpacing.l,
          runSpacing: AppSpacing.s,
          children: [
            _Stat(Icons.straighten, formatDistance(path.distanceM, unit), scheme),
            if (ascent != null && ascent > 0)
              _Stat(Icons.trending_up, '${ascent.round()} m', scheme),
            if (descent != null && descent > 0)
              _Stat(Icons.trending_down, '${descent.round()} m', scheme),
            _Stat(Icons.schedule, _formatDuration(etaSeconds), scheme),
          ],
        ),
        if (deviation != null) ...[
          const SizedBox(height: AppSpacing.s),
          _Stat(
            Icons.alt_route,
            'vs plan: ${(deviation.completionFraction * 100).round()}% '
                'completed · ${deviation.maxOffsetM.round()} m max off',
            scheme,
          ),
        ],
        if (hasProfile) ...[
          const SizedBox(height: AppSpacing.m),
          ElevationProfile(elevations: elev, unit: unit),
        ],
      ],
    );
  }

  static String _formatDuration(double seconds) {
    final total = seconds.round();
    final h = total ~/ 3600;
    final m = (total % 3600) ~/ 60;
    if (h > 0) return '~${h}h ${m}m';
    return '~${m}m';
  }
}

class _Stat extends StatelessWidget {
  final IconData icon;
  final String label;
  final ColorScheme scheme;
  const _Stat(this.icon, this.label, this.scheme);

  @override
  Widget build(BuildContext context) {
    return Row(
      mainAxisSize: MainAxisSize.min,
      children: [
        Icon(icon, size: 16, color: scheme.onSurfaceVariant),
        const SizedBox(width: 4),
        Text(label,
            style: Theme.of(context)
                .textTheme
                .bodyMedium
                ?.copyWith(color: scheme.onSurfaceVariant)),
      ],
    );
  }
}
