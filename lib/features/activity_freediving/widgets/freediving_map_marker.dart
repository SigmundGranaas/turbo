import 'package:flutter/material.dart';
import 'package:turbo/features/activities/api.dart';

class FreedivingMapMarker extends StatelessWidget {
  final ActivitySummary summary;
  const FreedivingMapMarker({super.key, required this.summary});

  @override
  Widget build(BuildContext context) => Tooltip(
    message: summary.name,
    child: const Icon(Icons.scuba_diving_outlined, color: Color(0xFF1565C0), size: 32,
      semanticLabel: 'Freediving spot'));
}
