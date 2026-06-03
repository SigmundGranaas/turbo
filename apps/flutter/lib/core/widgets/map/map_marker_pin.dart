import 'dart:math' as math;

import 'package:flutter/material.dart';
import 'package:turbo/app/shadows.dart';

/// The one map pin. A custom-painted "droplet" so every entity dropped on the
/// map — saved markers, activities, saved-path icons, search hits — reads as
/// the same marker type instead of each layer inventing its own (a black
/// `Icons.place`, a bare glyph, a coloured circle, …).
///
/// Pass an [icon] to show a glyph on a surface droplet; pass `null` for the
/// classic pin — a solid brand-coloured droplet with a white hole. Colours are
/// theme-correct and fixed where they must read on the always-light topo.
class MapMarkerPin extends StatefulWidget {
  /// The glyph inside the droplet. Null renders the icon-less "classic pin".
  final IconData? icon;

  /// Glyph tint (lets a layer colour-code by kind). Defaults to `onSurface`.
  final Color? accent;

  /// Optional hover label (desktop) — typically the entity name.
  final String? title;

  final VoidCallback? onTap;
  final VoidCallback? onLongPress;
  final bool isSelected;
  final double scale;

  static const double baseWidth = 40.0;
  static const double baseHeight = 60.0;
  static const double iconSize = 24.0;

  /// Fixed brand colour for the icon-less pin. Deliberately NOT the theme
  /// primary — that flips to a pale salmon in dark mode and disappears on the
  /// always-light topo basemap. This is the light-scheme brand terracotta.
  static const Color defaultPinColor = Color(0xFF8F4C38);

  const MapMarkerPin({
    super.key,
    this.icon,
    this.accent,
    this.title,
    this.onTap,
    this.onLongPress,
    this.isSelected = false,
    this.scale = 1.0,
  });

  @override
  State<MapMarkerPin> createState() => _MapMarkerPinState();
}

class _MapMarkerPinState extends State<MapMarkerPin> {
  bool _isHovering = false;

  @override
  Widget build(BuildContext context) {
    final colorScheme = Theme.of(context).colorScheme;
    final textTheme = Theme.of(context).textTheme;

    final width = MapMarkerPin.baseWidth * widget.scale;
    final height = MapMarkerPin.baseHeight * widget.scale;
    final iconSizeScaled = MapMarkerPin.iconSize * widget.scale;
    final circleRadius = width / 2;

    final hasIcon = widget.icon != null;
    final selected = widget.isSelected;
    final Color pinColor = selected
        ? colorScheme.secondaryContainer
        : (hasIcon ? colorScheme.surface : MapMarkerPin.defaultPinColor);
    final Color accentColor = selected
        ? colorScheme.onSecondaryContainer
        : (hasIcon ? (widget.accent ?? colorScheme.onSurface) : Colors.white);
    // White casing only for the solid-colour classic pin, so it reads cleanly
    // over busy terrain.
    final Color? pinBorder = (!hasIcon && !selected) ? Colors.white : null;

    final Widget pinContent = hasIcon
        ? Icon(widget.icon, color: accentColor, size: iconSizeScaled)
        : Container(
            width: iconSizeScaled * 0.5,
            height: iconSizeScaled * 0.5,
            decoration:
                BoxDecoration(shape: BoxShape.circle, color: accentColor),
          );

    final pinWidget = CustomPaint(
      size: Size(width, height),
      painter: DropletPainter(
        color: pinColor,
        scale: widget.scale,
        borderColor: pinBorder,
      ),
      child: Center(
        child: Padding(
          padding: EdgeInsets.only(bottom: height - (circleRadius * 2)),
          child: pinContent,
        ),
      ),
    );

    final title = widget.title;
    final labelWidget = (title == null)
        ? const SizedBox.shrink()
        : AnimatedOpacity(
            opacity: _isHovering ? 1.0 : 0.0,
            duration: const Duration(milliseconds: 150),
            child: Container(
              padding: EdgeInsets.symmetric(
                  horizontal: 8.0 * widget.scale, vertical: 4.0 * widget.scale),
              decoration: BoxDecoration(
                color: colorScheme.surfaceContainerHighest,
                borderRadius: BorderRadius.circular(8.0 * widget.scale),
                boxShadow: AppShadows.mapOverlay,
              ),
              child: Text(
                title,
                style: textTheme.labelMedium?.copyWith(
                  fontWeight: FontWeight.w500,
                  color: colorScheme.onSurface,
                  fontSize: (textTheme.labelMedium?.fontSize ?? 12) * widget.scale,
                ),
                maxLines: 1,
                overflow: TextOverflow.ellipsis,
              ),
            ),
          );

    return SizedBox(
      width: width,
      height: height,
      child: GestureDetector(
        onTap: widget.onTap,
        onLongPress: widget.onLongPress,
        child: MouseRegion(
          cursor: SystemMouseCursors.click,
          onEnter: (_) => setState(() => _isHovering = true),
          onExit: (_) => setState(() => _isHovering = false),
          child: Stack(
            clipBehavior: Clip.none,
            alignment: Alignment.topCenter,
            children: [
              pinWidget,
              if (title != null) Positioned(top: height, child: labelWidget),
            ],
          ),
        ),
      ),
    );
  }
}

/// Paints the [MapMarkerPin] teardrop. Optional [borderColor] draws a white
/// casing so the pin reads over busy terrain.
class DropletPainter extends CustomPainter {
  final Color color;
  final double scale;
  final Color? borderColor;
  final double borderWidth;

  DropletPainter({
    required this.color,
    this.scale = 1.0,
    this.borderColor,
    this.borderWidth = 2.0,
  });

  @override
  void paint(Canvas canvas, Size size) {
    final double w = size.width;
    final double h = size.height;
    final double circleRadius = w / 2;
    final Offset center = Offset(w / 2, circleRadius);

    final paintFill = Paint()
      ..color = color
      ..style = PaintingStyle.fill;

    final path = Path()
      ..moveTo(w / 2, h)
      ..cubicTo(w / 2, h * 0.80, 0, h * 0.65, 0, circleRadius)
      ..arcTo(Rect.fromCircle(center: center, radius: circleRadius), math.pi,
          math.pi, false)
      ..cubicTo(w, h * 0.65, w / 2, h * 0.80, w / 2, h)
      ..close();

    canvas.drawPath(path, paintFill);
    final border = borderColor;
    if (border != null) {
      canvas.drawPath(
        path,
        Paint()
          ..color = border
          ..style = PaintingStyle.stroke
          ..strokeWidth = borderWidth * scale,
      );
    }
  }

  @override
  bool shouldRepaint(covariant DropletPainter oldDelegate) {
    return oldDelegate.color != color ||
        oldDelegate.scale != scale ||
        oldDelegate.borderColor != borderColor ||
        oldDelegate.borderWidth != borderWidth;
  }
}
