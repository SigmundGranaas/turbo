import 'dart:async';
import 'package:flutter/material.dart';
import 'package:flutter_map/flutter_map.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:turbo/data/model/marker.dart' as marker_model;
import 'package:turbo/data/state/providers/location_repository.dart';

import '../../../data/icon_service.dart';
import '../../../data/model/named_icon.dart';
import '../../../data/state/providers/viewport_marker_provider.dart';
import '../../marker/edit_location_sheet.dart';

class ViewportMarkers extends ConsumerStatefulWidget {
  final MapController mapController;

  const ViewportMarkers({
    super.key,
    required this.mapController,
  });

  @override
  ConsumerState<ViewportMarkers> createState() => _ViewportMarkersState();
}

class _ViewportMarkersState extends ConsumerState<ViewportMarkers> {
  StreamSubscription<MapEvent>? _mapEventSubscription;
  ProviderSubscription<AsyncValue<List<marker_model.Marker>>>? _locationRepositorySubscription;
  bool _isMapReady = false; // Guard flag to ensure map is initialized

  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addPostFrameCallback((_) {
      // Listen to map events to know when the controller is ready
      _mapEventSubscription = widget.mapController.mapEventStream.listen((event) {
        if (event is MapEventMoveEnd ||
            event is MapEventRotateEnd ||
            event is MapEventFlingAnimationEnd ||
            event is MapEventDoubleTapZoomEnd ||
            event is MapEventScrollWheelZoom) {

          // The first event signifies the map is ready. Set the flag.
          if (!_isMapReady) {
            _isMapReady = true;
          }

          // Now it's safe to update markers.
          _updateViewportMarkers();
        }
      });

      // Listen to data changes (e.g., login, new marker)
      _locationRepositorySubscription = ref.listenManual(locationRepositoryProvider, (prev, next) {
        // IMPORTANT: Only update if the map is ready. This prevents the startup crash.
        if (_isMapReady) {
          _updateViewportMarkers();
        }
      });
    });
  }

  @override
  void dispose() {
    _mapEventSubscription?.cancel();
    _locationRepositorySubscription?.close(); // Use close for ProviderSubscription
    super.dispose();
  }

  void _updateViewportMarkers() {
    // A final safety check, though the listeners are now gated.
    if (!mounted || !_isMapReady) return;

    final bounds = widget.mapController.camera.visibleBounds;
    final zoom = widget.mapController.camera.zoom;
    ref.read(viewportMarkerNotifierProvider.notifier).loadMarkersInViewport(bounds, zoom);
  }

  @override
  Widget build(BuildContext context) {
    final viewportMarkersAsync = ref.watch(viewportMarkerNotifierProvider);
    final iconService = IconService();

    return viewportMarkersAsync.when(
      data: (locations) {
        if (widget.mapController.camera.zoom < 7 && locations.length > 50) {
        }
        return MarkerLayer(
          markers: locations.map((location) {
            final namedIcon = iconService.getIcon(location.icon);
            const double markerScale = 1.0;
            const double verticalOffset = 30.0 * markerScale;

            return Marker(
              width: 80.0,
              height: 90.0,
              point: location.position,
              alignment: Alignment.bottomCenter,
              rotate: false,
              child: Transform.translate(
                offset: const Offset(0, -verticalOffset / 2),
                child: MapMarkerWidget(
                  namedIcon: namedIcon,
                  title: location.title,
                  onTap: () => _showEditSheet(context, ref, location),
                  scale: markerScale,
                ),
              ),
            );
          }).toList(),
        );
      },
      loading: () {
        final previousData = ref.read(viewportMarkerNotifierProvider).asData?.value;
        if (previousData != null && previousData.isNotEmpty) {
          return MarkerLayer(
            markers: previousData.map((location) {
              final namedIcon = iconService.getIcon(location.icon);
              const double markerScale = 1.0;
              const double verticalOffset = 30.0 * markerScale;
              return Marker(
                width: 80.0, height: 90.0, point: location.position,
                alignment: Alignment.bottomCenter, rotate: false,
                child: Transform.translate(
                  offset: const Offset(0, -verticalOffset / 2),
                  child: MapMarkerWidget(namedIcon: namedIcon, title: location.title, onTap: () => _showEditSheet(context, ref, location), scale: markerScale),
                ),
              );
            }).toList(),
          );
        }
        return const SizedBox.shrink();
      },
      error: (error, stack) {
        debugPrint("Error loading viewport markers: $error\n$stack");
        return const SizedBox.shrink();
      },
    );
  }

  void _showEditSheet(BuildContext context, WidgetRef ref, marker_model.Marker marker) async {
    final result = await showModalBottomSheet<bool>(
      context: context,
      isScrollControlled: true,
      builder: (BuildContext context) {
        return EditLocationSheet(location: marker);
      },
    );
    if (result == true && mounted) {
      ref.read(viewportMarkerNotifierProvider.notifier).invalidateCache();
      _updateViewportMarkers();
    }
  }
}

class MapMarkerWidget extends StatefulWidget {
  final NamedIcon namedIcon;
  final String title;
  final VoidCallback onTap;
  final double scale;

  const MapMarkerWidget({
    super.key,
    required this.namedIcon,
    required this.title,
    required this.onTap,
    this.scale = 1.0,
  });

  @override
  State<MapMarkerWidget> createState() => _MapMarkerWidgetState();
}

class _MapMarkerWidgetState extends State<MapMarkerWidget> {
  bool _isHovering = false;
  bool _isPressed = false;

  static const double baseWidth = 38.0;
  static const double baseHeight = 50.0;
  static const double iconSize = 22.0;
  static const double iconPadding = -0.1;

  @override
  Widget build(BuildContext context) {
    final colorScheme = Theme.of(context).colorScheme;
    final textTheme = Theme.of(context).textTheme;

    final width = baseWidth * widget.scale;
    final height = baseHeight * widget.scale;
    final iconSizeScaled = iconSize * widget.scale;
    final iconPaddingScaled = iconPadding * widget.scale;

    return Column(
      mainAxisSize: MainAxisSize.min,
      children: [
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
              alignment: Alignment.topCenter,
              children: [
                CustomPaint(
                  size: Size(width, height),
                  painter: DropletPainter(
                    color: colorScheme.surface,
                    isPressed: _isPressed,
                    isHovered: _isHovering,
                    outlineColor: colorScheme.outline.withValues(alpha: 0.5),
                    scale: widget.scale,
                    drawDropletBorder: false,
                  ),
                ),
                Positioned(
                  top: iconPaddingScaled,
                  child: Container(
                    width: width - (iconPaddingScaled * 2),
                    height: width - (iconPaddingScaled * 2),
                    decoration: BoxDecoration(
                      color: colorScheme.surface,
                      shape: BoxShape.circle,
                    ),
                    alignment: Alignment.center,
                    child: Icon(
                      widget.namedIcon.icon,
                      color: colorScheme.primary,
                      size: iconSizeScaled,
                    ),
                  ),
                ),
              ],
            ),
          ),
        ),
        AnimatedOpacity(
          opacity: (_isHovering || _isPressed) ? 1.0 : 0.0,
          duration: const Duration(milliseconds: 200),
          child: Transform.translate(
            offset: Offset(0, 2 * widget.scale),
            child: Container(
              margin: EdgeInsets.only(top: 0.0 * widget.scale),
              padding: EdgeInsets.symmetric(
                horizontal: 6.0 * widget.scale,
                vertical: 2.0 * widget.scale,
              ),
              decoration: BoxDecoration(
                color: colorScheme.surfaceContainerHighest,
                borderRadius: BorderRadius.circular(4.0 * widget.scale),
                boxShadow: [
                  BoxShadow(
                    color: Colors.black.withValues(alpha: 0.1),
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
        ),
      ],
    );
  }
}

class DropletPainter extends CustomPainter {
  final Color color;
  final Color outlineColor;
  final bool isPressed;
  final bool isHovered;
  final double scale;
  final bool drawDropletBorder;

  DropletPainter({
    required this.color,
    required this.outlineColor,
    this.isPressed = false,
    this.isHovered = false,
    this.scale = 1.0,
    this.drawDropletBorder = true,
  });

  @override
  void paint(Canvas canvas, Size size) {
    final double w = size.width;
    final double h = size.height;
    final double circleRadius = w / 2;

    // Fill paint for droplet
    final paintFill = Paint()
      ..color = color
      ..style = PaintingStyle.fill;

    // Stroke paint for outline (only used if drawDropletBorder is true)
    final paintStroke = Paint()
      ..color = outlineColor
      ..style = PaintingStyle.stroke
      ..strokeWidth = (isPressed || isHovered ? 1.5 : 1.0) * scale;

    // Create droplet path with proper connection to circle
    final path = Path();

    // Start at the bottom of the droplet
    path.moveTo(w / 2, h);

    // Left curve of droplet - make it connect smoothly with the circle
    path.quadraticBezierTo(0, h * 0.7, 0, circleRadius);

    // Connect to the circle at the left side
    path.lineTo(0, circleRadius);

    // Draw only the bottom half of the "circle" as part of this path
    path.arcToPoint(
      Offset(w, circleRadius),
      radius: Radius.circular(circleRadius),
      clockwise: false,
    );

    // Right curve of droplet - make it connect smoothly with the circle
    path.quadraticBezierTo(w, h * 0.7, w / 2, h);

    path.close();

    // Draw shadow first
    canvas.drawShadow(path, Colors.black.withValues(alpha: 0.25), 2.5 * scale, true);

    // Draw filled droplet
    canvas.drawPath(path, paintFill);

    // Draw outline only if requested
    if (drawDropletBorder) {
      canvas.drawPath(path, paintStroke);
    }
  }

  @override
  bool shouldRepaint(covariant DropletPainter oldDelegate) {
    return oldDelegate.color != color ||
        oldDelegate.outlineColor != outlineColor ||
        oldDelegate.isPressed != isPressed ||
        oldDelegate.isHovered != isHovered ||
        oldDelegate.scale != scale ||
        oldDelegate.drawDropletBorder != drawDropletBorder;
  }
}