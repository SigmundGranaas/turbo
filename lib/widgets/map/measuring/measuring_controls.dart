import 'package:flutter/material.dart';

class MeasuringControls extends StatelessWidget {
  final double distance;
  final VoidCallback onReset;
  final VoidCallback onUndo;
  final VoidCallback onFinish;
  final bool canUndo;
  final bool canReset;

  const MeasuringControls({
    super.key,
    required this.distance,
    required this.onReset,
    required this.onUndo,
    required this.onFinish,
    required this.canUndo,
    required this.canReset,
  });

  @override
  Widget build(BuildContext context) {
    final colorScheme = Theme.of(context).colorScheme;
    final textTheme = Theme.of(context).textTheme;

    return Card(
      elevation: 4,
      margin: const EdgeInsets.symmetric(horizontal: 16),
      shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(28)),
      color: colorScheme.surfaceContainer,
      child: Padding(
        padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 12),
        child: Row(
          mainAxisSize: MainAxisSize.min,
          children: [
            Icon(Icons.straighten_outlined, color: colorScheme.onSurfaceVariant),
            const SizedBox(width: 12),
            Expanded(
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                mainAxisSize: MainAxisSize.min,
                children: [
                  Text(
                    'Total Distance',
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
            IconButton(
              onPressed: canUndo ? onUndo : null,
              icon: const Icon(Icons.undo),
              tooltip: 'Undo Last Point',
            ),
            IconButton(
              onPressed: canReset ? onReset : null,
              icon: const Icon(Icons.delete_sweep_outlined),
              tooltip: 'Reset Measurement',
            ),
            const SizedBox(width: 8),
            FilledButton.tonal(
              onPressed: onFinish,
              style: FilledButton.styleFrom(
                padding: const EdgeInsets.symmetric(horizontal: 20),
              ),
              child: const Text('Done'),
            ),
          ],
        ),
      ),
    );
  }
}