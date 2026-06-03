import 'package:flutter/material.dart';
import 'package:turbo/app/tokens.dart';

/// The app's single drag-handle pill (32×4, `outlineVariant`, 2dp radius),
/// centred at the top of a sheet. Shared so every sheet — modal sheets via
/// `showAppSheet` and the floating map-tool panels (route planning, measuring)
/// — reads as the same sheet semantic, instead of each inventing its own
/// affordance.
class SheetDragHandle extends StatelessWidget {
  const SheetDragHandle({super.key});

  @override
  Widget build(BuildContext context) {
    return Center(
      child: Container(
        width: 32,
        height: 4,
        margin: const EdgeInsets.symmetric(vertical: AppSpacing.s),
        decoration: BoxDecoration(
          color: Theme.of(context).colorScheme.outlineVariant,
          borderRadius: const BorderRadius.all(Radius.circular(2)),
        ),
      ),
    );
  }
}
