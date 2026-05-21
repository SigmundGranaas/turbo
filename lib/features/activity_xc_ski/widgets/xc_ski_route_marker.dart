import 'package:flutter/material.dart';
import 'package:turbo/features/activities/api.dart';

class XcSkiRouteMarker extends StatelessWidget {
  final ActivitySummary summary;
  const XcSkiRouteMarker({super.key, required this.summary});

  @override
  Widget build(BuildContext context) => Tooltip(
    message: summary.name,
    child: const Icon(Icons.snowshoeing_outlined, color: Color(0xFF00838F), size: 32,
      semanticLabel: 'XC ski trail'));
}
