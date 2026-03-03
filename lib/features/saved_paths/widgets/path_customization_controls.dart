import 'package:flutter/material.dart';
import 'package:turbo/core/widgets/color_circle.dart';
import 'package:turbo/l10n/app_localizations.dart';
import 'package:turbo/features/markers/data/icon_service.dart';
import 'package:turbo/features/markers/widgets/icon_selection_page.dart';
import '../models/path_style.dart';

class PathCustomizationControls extends StatelessWidget {
  final Color? selectedColor;
  final ValueChanged<Color?> onColorChanged;
  final String? selectedIconKey;
  final ValueChanged<String?> onIconChanged;
  final bool isSmoothing;
  final ValueChanged<bool> onSmoothingChanged;
  final PathLineStyle lineStyle;
  final ValueChanged<PathLineStyle> onLineStyleChanged;
  final bool initiallyExpanded;

  const PathCustomizationControls({
    super.key,
    required this.selectedColor,
    required this.onColorChanged,
    required this.selectedIconKey,
    required this.onIconChanged,
    required this.isSmoothing,
    required this.onSmoothingChanged,
    required this.lineStyle,
    required this.onLineStyleChanged,
    this.initiallyExpanded = false,
  });

  @override
  Widget build(BuildContext context) {
    final l10n = context.l10n;

    return ExpansionTile(
      title: Text(l10n.appearance),
      tilePadding: EdgeInsets.zero,
      initiallyExpanded: initiallyExpanded,
      children: [
        _buildColorRow(context, l10n),
        const SizedBox(height: 12),
        _buildIconRow(context, l10n),
        SwitchListTile(
          title: Text(l10n.pathSmoothing),
          value: isSmoothing,
          onChanged: onSmoothingChanged,
          contentPadding: EdgeInsets.zero,
        ),
        _buildLineStyleRow(context),
      ],
    );
  }

  Widget _buildColorRow(BuildContext context, AppLocalizations l10n) {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Text(l10n.pathColor, style: Theme.of(context).textTheme.titleSmall),
        const SizedBox(height: 8),
        SingleChildScrollView(
          scrollDirection: Axis.horizontal,
          child: Row(
            children: [
              ColorCircle(
                color: null,
                isSelected: selectedColor == null,
                onTap: () => onColorChanged(null),
                label: l10n.defaultColor,
                colorScheme: Theme.of(context).colorScheme,
              ),
              ...pathColorPalette.map((color) => ColorCircle(
                    color: color,
                    isSelected: selectedColor != null &&
                        colorToHex(selectedColor!) == colorToHex(color),
                    onTap: () => onColorChanged(color),
                    colorScheme: Theme.of(context).colorScheme,
                  )),
            ],
          ),
        ),
      ],
    );
  }

  Widget _buildIconRow(BuildContext context, AppLocalizations l10n) {
    final iconService = IconService();
    final hasIcon = selectedIconKey != null;
    final namedIcon =
        hasIcon ? iconService.getIcon(context, selectedIconKey) : null;
    final colorScheme = Theme.of(context).colorScheme;
    final textTheme = Theme.of(context).textTheme;

    return Material(
      color: colorScheme.surfaceContainer,
      shape: RoundedRectangleBorder(
        borderRadius: BorderRadius.circular(16),
      ),
      clipBehavior: Clip.antiAlias,
      child: InkWell(
        onTap: () async {
          final result = await IconSelectionPage.show(context);
          FocusManager.instance.primaryFocus?.unfocus();
          if (result != null) {
            onIconChanged(result.title);
          }
        },
        child: Padding(
          padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 12),
          child: Row(
            children: [
              Container(
                width: 40,
                height: 40,
                decoration: BoxDecoration(
                  color: hasIcon
                      ? colorScheme.primaryContainer
                      : colorScheme.surfaceContainerHighest,
                  shape: BoxShape.circle,
                ),
                child: Icon(
                  hasIcon ? namedIcon!.icon : Icons.add,
                  color: hasIcon
                      ? colorScheme.onPrimaryContainer
                      : colorScheme.onSurfaceVariant,
                  size: 24,
                ),
              ),
              const SizedBox(width: 16),
              Expanded(
                child: Text(
                  hasIcon
                      ? (namedIcon!.localizedTitle ?? namedIcon.title)
                      : l10n.pathIcon,
                  style: textTheme.bodyLarge?.copyWith(
                    color: colorScheme.onSurface,
                  ),
                ),
              ),
              if (hasIcon)
                IconButton(
                  icon: const Icon(Icons.clear),
                  tooltip: l10n.removeIcon,
                  onPressed: () => onIconChanged(null),
                )
              else
                Icon(
                  Icons.chevron_right,
                  color: colorScheme.onSurfaceVariant,
                ),
            ],
          ),
        ),
      ),
    );
  }

  Widget _buildLineStyleRow(BuildContext context) {
    final color = Theme.of(context).colorScheme.onSurface;

    return SizedBox(
      width: double.infinity,
      child: SegmentedButton<PathLineStyle>(
        segments: PathLineStyle.values.map((style) => ButtonSegment(
              value: style,
              icon: CustomPaint(
                size: const Size(40, 2),
                painter: _LinePatternPainter(style, color),
              ),
            )).toList(),
        selected: {lineStyle},
        onSelectionChanged: (selected) =>
            onLineStyleChanged(selected.first),
      ),
    );
  }
}

class _LinePatternPainter extends CustomPainter {
  final PathLineStyle style;
  final Color color;

  _LinePatternPainter(this.style, this.color);

  @override
  void paint(Canvas canvas, Size size) {
    final paint = Paint()
      ..color = color
      ..strokeWidth = 2
      ..strokeCap = StrokeCap.round;

    final y = size.height / 2;

    switch (style) {
      case PathLineStyle.solid:
        canvas.drawLine(Offset(0, y), Offset(size.width, y), paint);
      case PathLineStyle.dotted:
        const spacing = 6.0;
        for (double x = 0; x <= size.width; x += spacing) {
          canvas.drawCircle(Offset(x, y), 1.2, paint);
        }
      case PathLineStyle.dashed:
        const dashWidth = 8.0;
        const gap = 4.0;
        double x = 0;
        while (x < size.width) {
          final end = (x + dashWidth).clamp(0.0, size.width);
          canvas.drawLine(Offset(x, y), Offset(end, y), paint);
          x += dashWidth + gap;
        }
      case PathLineStyle.dashDot:
        const dashWidth = 8.0;
        const gap = 4.0;
        double x = 0;
        bool isDash = true;
        while (x < size.width) {
          if (isDash) {
            final end = (x + dashWidth).clamp(0.0, size.width);
            canvas.drawLine(Offset(x, y), Offset(end, y), paint);
            x += dashWidth + gap;
          } else {
            canvas.drawCircle(Offset(x, y), 1.2, paint);
            x += gap;
          }
          isDash = !isDash;
        }
    }
  }

  @override
  bool shouldRepaint(_LinePatternPainter oldDelegate) =>
      style != oldDelegate.style || color != oldDelegate.color;
}

