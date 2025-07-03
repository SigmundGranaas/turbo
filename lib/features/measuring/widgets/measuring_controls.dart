import 'package:flutter/material.dart';
import 'package:turbo/l10n/app_localizations.dart';

class MeasuringControls extends StatelessWidget {
  final double distance;
  final VoidCallback onReset;
  final VoidCallback onUndo;
  final VoidCallback onFinish;
  final VoidCallback onToggleSmoothing;
  final VoidCallback onToggleDrawing;
  final VoidCallback onToggleIntermediatePoints;
  final Function(double) onSensitivityChanged;
  final bool canUndo;
  final bool canReset;
  final bool isSmoothing;
  final bool isDrawing;
  final bool showIntermediatePoints;
  final double drawSensitivity;

  const MeasuringControls({
    super.key,
    required this.distance,
    required this.onReset,
    required this.onUndo,
    required this.onFinish,
    required this.onToggleSmoothing,
    required this.onToggleDrawing,
    required this.onToggleIntermediatePoints,
    required this.onSensitivityChanged,
    required this.canUndo,
    required this.canReset,
    required this.isSmoothing,
    required this.isDrawing,
    required this.showIntermediatePoints,
    required this.drawSensitivity,
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

    final actionButtons = [
      IconButton(
        onPressed: onToggleDrawing,
        icon: const Icon(Icons.draw_outlined),
        tooltip: l10n.drawMode,
        style: isDrawing ? selectedStyle : null,
      ),
      IconButton(
        onPressed: onToggleSmoothing,
        icon: const Icon(Icons.insights_outlined),
        tooltip: l10n.smoothLine,
        style: isSmoothing ? selectedStyle : null,
      ),
      IconButton(
        onPressed: canReset ? onToggleIntermediatePoints : null,
        icon: const Icon(Icons.linear_scale_outlined),
        tooltip: l10n.toggleIntermediatePoints,
        style: showIntermediatePoints ? null : selectedStyle,
      ),
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
    ];

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
            // Top row: Distance display and Done button
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
                    onPressed: onFinish,
                    style: FilledButton.styleFrom(
                      padding: const EdgeInsets.symmetric(horizontal: 20),
                    ),
                    child: Text(l10n.done),
                  ),
                ],
              ),
            ),
            // Divider if there are action buttons or slider
            if (actionButtons.isNotEmpty || isDrawing)
              const Divider(height: 1, indent: 16, endIndent: 16),
            // Middle row: Action buttons
            Padding(
              padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 4),
              child: Wrap(
                alignment: WrapAlignment.center,
                spacing: 8.0, // Horizontal space between buttons
                runSpacing: 4.0, // Vertical space if it wraps
                children: actionButtons,
              ),
            ),
            // Bottom row: Slider for drawing sensitivity
            if (isDrawing)
              Padding(
                padding:
                const EdgeInsets.only(left: 16, right: 16, bottom: 8),
                child: Row(
                  children: [
                    const Icon(Icons.line_axis, size: 20),
                    Expanded(
                      child: Slider(
                        value: drawSensitivity,
                        min: 5,
                        max: 50,
                        divisions: 9,
                        label: drawSensitivity.round().toString(),
                        onChanged: onSensitivityChanged,
                      ),
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