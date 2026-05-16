import 'package:flutter/material.dart';
import 'package:turbo/app/tokens.dart';
import 'package:turbo/app/l10n/app_localizations.dart';

/// Action sheet shown when the user long-presses the map.
///
/// Three rows: create marker, measure distance, navigate to / stop navigating.
/// The "navigate" row reads as "Stop navigation" when [isNavigating] is true.
///
/// Extracted from [MainMapPage] so the flow can be widget-tested without
/// pumping the full map.
class PinOptionsSheet extends StatelessWidget {
  final bool isNavigating;
  final VoidCallback onCreateMarker;
  final VoidCallback onMeasure;
  final VoidCallback onNavigate;
  final VoidCallback onStopNavigation;

  const PinOptionsSheet({
    super.key,
    required this.isNavigating,
    required this.onCreateMarker,
    required this.onMeasure,
    required this.onNavigate,
    required this.onStopNavigation,
  });

  @override
  Widget build(BuildContext context) {
    final l10n = context.l10n;
    return SafeArea(
      child: Container(
        padding: const EdgeInsets.all(AppSpacing.l),
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            ListTile(
              leading: const Icon(Icons.add_location_alt_outlined),
              title: Text(l10n.createNewMarkerHere),
              onTap: () {
                Navigator.pop(context);
                onCreateMarker();
              },
            ),
            ListTile(
              leading: const Icon(Icons.straighten),
              title: Text(l10n.measureDistanceFromHere),
              onTap: () {
                Navigator.pop(context);
                onMeasure();
              },
            ),
            ListTile(
              leading: const Icon(Icons.navigation_outlined),
              title: Text(isNavigating
                  ? l10n.stopNavigation
                  : l10n.navigateToHere),
              onTap: () {
                Navigator.pop(context);
                if (isNavigating) {
                  onStopNavigation();
                } else {
                  onNavigate();
                }
              },
            ),
          ],
        ),
      ),
    );
  }
}
