import 'package:flutter/material.dart';

class ColorCircle extends StatelessWidget {
  final Color? color;
  final bool isSelected;
  final VoidCallback onTap;
  final String? label;
  final ColorScheme colorScheme;

  const ColorCircle({
    super.key,
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

  static Color contrastColor(Color color) {
    final luminance = color.computeLuminance();
    return luminance > 0.5 ? Colors.black : Colors.white;
  }

  Color _contrastColor(Color color) => contrastColor(color);
}
