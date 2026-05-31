import 'package:flutter/material.dart';
import 'package:turbo/features/activities/api.dart';

class PackraftingRouteMarker extends StatelessWidget {
  final ActivitySummary summary;
  const PackraftingRouteMarker({super.key, required this.summary});

  @override
  Widget build(BuildContext context) => Tooltip(
    message: summary.name,
    child: const Icon(Icons.kayaking_outlined, color: Color(0xFF00838F), size: 32,
      semanticLabel: 'Packrafting trip'));
}
