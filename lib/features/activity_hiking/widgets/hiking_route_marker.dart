import 'package:flutter/material.dart';
import 'package:turbo/features/activities/api.dart';

class HikingRouteMarker extends StatelessWidget {
  final ActivitySummary summary;
  const HikingRouteMarker({super.key, required this.summary});

  @override
  Widget build(BuildContext context) => Tooltip(
    message: summary.name,
    child: const Icon(Icons.hiking_outlined, color: Color(0xFF2E7D32), size: 32,
      semanticLabel: 'Hiking trail'));
}
