import 'package:flutter/material.dart';
import 'package:turbo/l10n/app_localizations.dart';

class MeasuringControls extends StatelessWidget {
  final double distance;
  final VoidCallback onReset;
  final VoidCallback onUndo;
  final VoidCallback onFinish;
  final VoidCallback onToggleDrawing;
  final bool canUndo;
  final bool canReset;
  final bool canSave;
  final bool isDrawing;

  const MeasuringControls({
    super.key,
    required this.distance,
    required this.onReset,
    required this.onUndo,
    required this.onFinish,
    required this.onToggleDrawing,
    required this.canUndo,
    required this.canReset,
    required this.canSave,
    required this.isDrawing,
  });

  @override
  Widget build(BuildContext context) {
    final l10n = context.l10n;
    final colorScheme = Theme.of(context).colorScheme;
    final textTheme = Theme.of(context).textTheme;

    final selectedStyle = IconButton.styleFrom(
      backgroundColor: colorScheme.primaryContainer,
      foregroundColor: colorScheme.onPrimaryContainer,
    );

    return Card(
      elevation: 4,
      margin: const EdgeInsets.symmetric(horizontal: 16),
      shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(28)),
      color: colorScheme.surfaceContainer,
      child: ConstrainedBox(
        constraints: const BoxConstraints(maxWidth: 700),
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            // Row 1: Distance display and Save button
            Padding(
              padding: const EdgeInsets.fromLTRB(16, 12, 16, 12),
              child: Row(
                children: [
                  Icon(Icons.straighten_outlined,
                      color: colorScheme.onSurfaceVariant),
                  const SizedBox(width: 12),
                  Expanded(
                    child: Column(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      mainAxisSize: MainAxisSize.min,
                      children: [
                        Text(
                          l10n.totalDistance,
                          style: textTheme.bodySmall?.copyWith(
                            color: colorScheme.onSurfaceVariant,
                          ),
                        ),
                        Text(
                          '${(distance / 1000).toStringAsFixed(2)} km',
                          style: textTheme.titleMedium?.copyWith(
                            fontWeight: FontWeight.bold,
                            color: colorScheme.onSurface,
                          ),
                        ),
                      ],
                    ),
                  ),
                  const SizedBox(width: 8),
                  FilledButton.tonal(
                    onPressed: canSave ? onFinish : null,
                    style: FilledButton.styleFrom(
                      padding: const EdgeInsets.symmetric(horizontal: 20),
                    ),
                    child: Text(l10n.save),
                  ),
                ],
              ),
            ),
            const Divider(height: 1, indent: 16, endIndent: 16),
            // Bottom row: Draw mode (left) | undo/reset (right)
            Padding(
              padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 4),
              child: Row(
                children: [
                  IconButton(
                    onPressed: onToggleDrawing,
                    icon: const Icon(Icons.draw_outlined),
                    tooltip: l10n.drawMode,
                    style: isDrawing ? selectedStyle : null,
                  ),
                  const Spacer(),
                  IconButton(
                    onPressed: canUndo ? onUndo : null,
                    icon: const Icon(Icons.undo),
                    tooltip: l10n.undoLastPoint,
                  ),
                  IconButton(
                    onPressed: canReset ? onReset : null,
                    icon: const Icon(Icons.delete_sweep_outlined),
                    tooltip: l10n.resetMeasurement,
                  ),
                ],
              ),
            ),
          ],
        ),
      ),
    );
  }
}
