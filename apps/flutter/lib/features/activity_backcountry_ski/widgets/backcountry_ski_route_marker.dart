import 'package:flutter/material.dart';

import 'package:turbo/features/activities/api.dart';

/// On-map representation. The cross-kind summaries layer renders a
/// `MarkerLayer`, so for a route we surface a single marker at the start
/// point with the kind's tint and icon. The full polyline rendering is a
/// follow-up that integrates with the map layer's overlay stack.
class BackcountrySkiRouteMarker extends StatelessWidget {
  final ActivitySummary summary;
  const BackcountrySkiRouteMarker({super.key, required this.summary});

  @override
  Widget build(BuildContext context) {
    return Tooltip(
      message: summary.name,
      child: const Icon(
        Icons.downhill_skiing_outlined,
        color: Color(0xFF7A3CCB),
        size: 32,
        semanticLabel: 'Backcountry ski route',
      ),
    );
  }
}
