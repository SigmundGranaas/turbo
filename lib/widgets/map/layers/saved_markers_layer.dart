import 'package:flutter/material.dart';
import 'package:flutter_map/flutter_map.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:map_app/data/model/marker.dart' as marker_model;

import '../../../data/icon_service.dart';
import '../../../data/model/named_icon.dart';
import '../../../data/state/providers/location_provider.dart';
import '../../marker/edit_location_sheet.dart';

class LocationMarkers extends ConsumerWidget {
  final Function(marker_model.Marker) onMarkerTap;

  const LocationMarkers({super.key, required this.onMarkerTap});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final locationsAsync = ref.watch(locationNotifierProvider);
    final iconService = IconService();

    return locationsAsync.when(
      data: (locations) => MarkerLayer(
        markers: locations.map((location) {
          final namedIcon = iconService.getIcon(location.icon);
          // Define marker scale
          const double markerScale = 1.0;
          // Calculate the offset to adjust marker position
          const double verticalOffset = 30.0 * markerScale; // Half of the marker height

          return Marker(
            width: 80.0,
            height: 90.0,
            point: location.position,
            alignment: Alignment.bottomCenter,
            // Offset the marker up by half its height so the point is at the exact location
            rotate: false,
            child: Transform.translate(
              offset: const Offset(0, -verticalOffset),
              child: MapMarker(
                namedIcon: namedIcon,
                title: location.title,
                onTap: () => _showEditSheet(context, ref, location),
                scale: markerScale,
              ),
            ),
          );
        }).toList(),
      ),
      loading: () => const MarkerLayer(markers: []),
      error: (error, stack) => const MarkerLayer(markers: []),
    );
  }

  void _showEditSheet(BuildContext context, WidgetRef ref, marker_model.Marker marker) async {
    await showModalBottomSheet(
      context: context,
      isScrollControlled: true,
      builder: (BuildContext context) {
        return EditLocationSheet(
          location: marker,
        );
      },
    );
  }
}

class MapMarker extends StatefulWidget {
  final NamedIcon namedIcon;
  final String title;
  final VoidCallback onTap;
  final double scale;

  const MapMarker({
    super.key,
    required this.namedIcon,
    required this.title,
    required this.onTap,
    this.scale = 1.0,
  });

  @override
  State<MapMarker> createState() => _MapMarkerState();
}

class _MapMarkerState extends State<MapMarker> {
  bool _isHovering = false;
  bool _isPressed = false;

  static const double baseWidth = 38.0;
  static const double baseHeight = 50.0;
  static const double iconSize = 22.0;
  static const double iconPadding = 2.0;

  @override
  Widget build(BuildContext context) {
    final colorScheme = Theme.of(context).colorScheme;
    final textTheme = Theme.of(context).textTheme;

    // Calculate scaled dimensions
    final width = baseWidth * widget.scale;
    final height = baseHeight * widget.scale;
    final iconSizeScaled = iconSize * widget.scale;
    final iconPaddingScaled = iconPadding * widget.scale;

    return Column(
      mainAxisSize: MainAxisSize.min,
      children: [
        // Marker with droplet shape - fixed position regardless of hover state
        GestureDetector(
          onTapDown: (_) => setState(() => _isPressed = true),
          onTapUp: (_) => setState(() => _isPressed = false),
          onTapCancel: () => setState(() => _isPressed = false),
          onTap: widget.onTap,
          child: MouseRegion(
            cursor: SystemMouseCursors.click,
            onEnter: (_) => setState(() => _isHovering = true),
            onExit: (_) => setState(() => _isHovering = false),
            child: Stack(
              clipBehavior: Clip.none,
              children: [
                // Droplet shape
                CustomPaint(
                  size: Size(width, height),
                  painter: DropletPainter(
                    color: colorScheme.surface, // Surface color for the base
                    isPressed: _isPressed,
                    isHovered: _isHovering,
                    outlineColor: colorScheme.outline.withOpacity(0.5),
                    scale: widget.scale,
                  ),
                ),

                // Icon positioned inside the top circular part of the droplet
                Positioned(
                  top: iconPaddingScaled,
                  left: iconPaddingScaled,
                  child: Container(
                    width: width - (iconPaddingScaled * 2),
                    height: width - (iconPaddingScaled * 2),
                    decoration: BoxDecoration(
                      color: colorScheme.surface, // Surface color for the icon background
                      shape: BoxShape.circle,
                    ),
                    alignment: Alignment.center,
                    child: Icon(
                      widget.namedIcon.icon,
                      color: colorScheme.primary, // Primary color for the icon itself
                      size: iconSizeScaled,
                    ),
                  ),
                ),
              ],
            ),
          ),
        ),

        // Label always appears below the marker, visibility controlled by state
        AnimatedOpacity(
          opacity: (_isHovering || _isPressed) ? 1.0 : 0.0,
          duration: const Duration(milliseconds: 200),
          child: Container(
            margin: EdgeInsets.only(top: 4.0 * widget.scale),
            padding: EdgeInsets.symmetric(
              horizontal: 6.0 * widget.scale,
              vertical: 2.0 * widget.scale,
            ),
            decoration: BoxDecoration(
              color: colorScheme.surfaceContainerHighest,
              borderRadius: BorderRadius.circular(4.0 * widget.scale),
              boxShadow: [
                BoxShadow(
                  color: Colors.black.withOpacity(0.1),
                  blurRadius: 2.0 * widget.scale,
                  offset: Offset(0, 1.0 * widget.scale),
                ),
              ],
            ),
            child: Text(
              widget.title,
              style: textTheme.labelSmall?.copyWith(
                fontWeight: FontWeight.w500,
                color: colorScheme.onSurface,
                fontSize: (textTheme.labelSmall?.fontSize ?? 10) * widget.scale,
              ),
              maxLines: 1,
              overflow: TextOverflow.ellipsis,
            ),
          ),
        ),
      ],
    );
  }
}

// Custom painter to draw the droplet shape
class DropletPainter extends CustomPainter {
  final Color color;
  final Color outlineColor;
  final bool isPressed;
  final bool isHovered;
  final double scale;

  DropletPainter({
    required this.color,
    required this.outlineColor,
    this.isPressed = false,
    this.isHovered = false,
    this.scale = 1.0,
  });

  @override
  void paint(Canvas canvas, Size size) {
    final double width = size.width;
    final double height = size.height;
    final double circleRadius = width / 2;

    final paintFill = Paint()
      ..color = color
      ..style = PaintingStyle.fill;

    final paintStroke = Paint()
      ..color = outlineColor
      ..style = PaintingStyle.stroke
      ..strokeWidth = (isPressed || isHovered ? 1.0 : 0.5) * scale;

    final path = Path();

    // Start at the bottom point
    path.moveTo(width / 2, height);

    // Draw left curve
    path.quadraticBezierTo(0, height * 0.6, 0, height * 0.4);

    // Draw top-left curve of the circle
    path.quadraticBezierTo(0, 0, circleRadius, 0);

    // Draw top-right curve of the circle
    path.quadraticBezierTo(width, 0, width, height * 0.4);

    // Draw right curve
    path.quadraticBezierTo(width, height * 0.6, width / 2, height);

    // Draw the fill and stroke
    canvas.drawShadow(path, Colors.black.withOpacity(0.2), 2.0 * scale, true);
    canvas.drawPath(path, paintFill);
    canvas.drawPath(path, paintStroke);
  }

  @override
  bool shouldRepaint(covariant DropletPainter oldDelegate) {
    return oldDelegate.color != color ||
        oldDelegate.outlineColor != outlineColor ||
        oldDelegate.isPressed != isPressed ||
        oldDelegate.isHovered != isHovered ||
        oldDelegate.scale != scale;
  }
}