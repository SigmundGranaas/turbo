import 'package:flutter/material.dart';
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
              _ColorCircle(
                color: null,
                isSelected: selectedColor == null,
                onTap: () => onColorChanged(null),
                label: l10n.defaultColor,
                colorScheme: Theme.of(context).colorScheme,
              ),
              ...pathColorPalette.map((color) => _ColorCircle(
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

    return ListTile(
      contentPadding: EdgeInsets.zero,
      leading: Icon(
        hasIcon ? namedIcon!.icon : Icons.add_circle_outline,
        color: hasIcon ? Theme.of(context).colorScheme.primary : null,
      ),
      title: Text(hasIcon
          ? (namedIcon!.localizedTitle ?? namedIcon.title)
          : l10n.pathIcon),
      trailing: hasIcon
          ? IconButton(
              icon: const Icon(Icons.clear),
              tooltip: l10n.removeIcon,
              onPressed: () => onIconChanged(null),
            )
          : null,
      onTap: () async {
        final result = await IconSelectionPage.show(context);
        if (result != null) {
          onIconChanged(result.title);
        }
      },
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

class _ColorCircle extends StatelessWidget {
  final Color? color;
  final bool isSelected;
  final VoidCallback onTap;
  final String? label;
  final ColorScheme colorScheme;

  const _ColorCircle({
    required this.color,
    required this.isSelected,
    required this.onTap,
    this.label,
    required this.colorScheme,
  });

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.only(right: 8),
      child: Tooltip(
        message: label ?? '',
        child: GestureDetector(
          onTap: onTap,
          child: Container(
            width: 36,
            height: 36,
            decoration: BoxDecoration(
              shape: BoxShape.circle,
              color: color ?? colorScheme.surfaceContainerHighest,
              border: color == null
                  ? Border.all(color: colorScheme.outline, width: 2)
                  : null,
            ),
            child: isSelected
                ? Icon(
                    Icons.check,
                    size: 20,
                    color: color == null
                        ? colorScheme.onSurface
                        : _contrastColor(color!),
                  )
                : null,
          ),
        ),
      ),
    );
  }

  Color _contrastColor(Color color) {
    final luminance = color.computeLuminance();
    return luminance > 0.5 ? Colors.black : Colors.white;
  }
}
