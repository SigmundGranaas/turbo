import 'package:flutter/material.dart';
import 'package:turbo/features/activities/api.dart';

/// On-map representation for a fishing summary. The shell calls this
/// from its map layer via the descriptor — no kind-specific switch in
/// the shell.
class FishingMapMarker extends StatelessWidget {
  final ActivitySummary summary;
  const FishingMapMarker({super.key, required this.summary});

  @override
  Widget build(BuildContext context) {
    return Tooltip(
      message: summary.name,
      child: const Icon(
        Icons.set_meal_outlined,
        color: Color(0xFF1E6FB8),
        size: 32,
        semanticLabel: 'Fishing spot',
      ),
    );
  }
}
