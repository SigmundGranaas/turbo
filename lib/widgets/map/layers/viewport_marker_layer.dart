import 'dart:async';
import 'dart:math' as math;
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
  ProviderSubscription<AsyncValue<List<marker_model.Marker>>>?
  _locationRepositorySubscription;
  bool _isMapReady = false;

  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addPostFrameCallback((_) {
      _mapEventSubscription =
          widget.mapController.mapEventStream.listen((event) {
            if (event is MapEventMoveEnd ||
                event is MapEventRotateEnd ||
                event is MapEventFlingAnimationEnd ||
                event is MapEventDoubleTapZoomEnd ||
                event is MapEventScrollWheelZoom) {
              if (!_isMapReady) {
                _isMapReady = true;
              }
              _updateViewportMarkers();
            }
          });

      _locationRepositorySubscription =
          ref.listenManual(locationRepositoryProvider, (prev, next) {
            if (_isMapReady) {
              _updateViewportMarkers();
            }
          });
    });
  }

  @override
  void dispose() {
    _mapEventSubscription?.cancel();
    _locationRepositorySubscription?.close();
    super.dispose();
  }

  void _updateViewportMarkers() {
    if (!mounted || !_isMapReady) return;

    final bounds = widget.mapController.camera.visibleBounds;
    final zoom = widget.mapController.camera.zoom;
    ref
        .read(viewportMarkerNotifierProvider.notifier)
        .loadMarkersInViewport(bounds, zoom);
  }

  @override
  Widget build(BuildContext context) {
    final viewportMarkersAsync = ref.watch(viewportMarkerNotifierProvider);
    final iconService = IconService();

    return viewportMarkersAsync.when(
      data: (locations) {
        return MarkerLayer(
          markers: locations.map((location) {
            final namedIcon = iconService.getIcon(context ,location.icon);
            const double markerScale = 1.0;
            const double markerHeight = (MapMarkerWidget.baseHeight * markerScale);

            return Marker(
              width: MapMarkerWidget.baseWidth * markerScale,
              height: markerHeight,
              point: location.position,
              alignment: Alignment.bottomCenter,
              child: Transform.translate(
                offset: const Offset(0, -markerHeight),
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
        final previousData =
            ref.read(viewportMarkerNotifierProvider).asData?.value;
        if (previousData != null && previousData.isNotEmpty) {
          return MarkerLayer(
            markers: previousData.map((location) {
              final namedIcon = iconService.getIcon(context, location.icon);
              const double markerScale = 1.0;
              const double markerHeight = MapMarkerWidget.baseHeight * markerScale;
              return Marker(
                width: MapMarkerWidget.baseWidth * markerScale,
                height: markerHeight,
                point: location.position,
                alignment: Alignment.bottomCenter,
                child: Transform.translate(
                  offset: Offset(0, -markerHeight / 2),
                  child: MapMarkerWidget(
                      namedIcon: namedIcon,
                      title: location.title,
                      onTap: () => _showEditSheet(context, ref, location),
                      scale: markerScale),
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

  void _showEditSheet(
      BuildContext context, WidgetRef ref, marker_model.Marker marker) async {
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

  static const double baseWidth = 40.0;
  static const double baseHeight = 60.0; // Adjusted for a taller, more classic look
  static const double iconSize = 24.0;

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

  @override
  Widget build(BuildContext context) {
    final colorScheme = Theme.of(context).colorScheme;
    final textTheme = Theme.of(context).textTheme;

    final width = MapMarkerWidget.baseWidth * widget.scale;
    final height = MapMarkerWidget.baseHeight * widget.scale;
    final iconSizeScaled = MapMarkerWidget.iconSize * widget.scale;
    final circleRadius = width / 2;

    final pinWidget = CustomPaint(
      size: Size(width, height),
      painter: DropletPainter(
        color: colorScheme.surface,
      ),
      child: Center(
        child: Padding(
          padding: EdgeInsets.only(bottom: height - (circleRadius * 2)),
          child: Icon(
            widget.namedIcon.icon,
            color: colorScheme.primary,
            size: iconSizeScaled,
          ),
        ),
      ),
    );

    final labelWidget = AnimatedOpacity(
      opacity: _isHovering ? 1.0 : 0.0,
      duration: const Duration(milliseconds: 150),
      child: Container(
        padding: EdgeInsets.symmetric(
          horizontal: 8.0 * widget.scale,
          vertical: 4.0 * widget.scale,
        ),
        decoration: BoxDecoration(
          color: colorScheme.surfaceContainerHighest,
          borderRadius: BorderRadius.circular(8.0 * widget.scale),
          boxShadow: [
            BoxShadow(
              color: Colors.black.withValues(alpha: 0.15),
              blurRadius: 4.0 * widget.scale,
              offset: Offset(0, 2.0 * widget.scale),
            ),
          ],
        ),
        child: Text(
          widget.title,
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
        child: MouseRegion(
          cursor: SystemMouseCursors.click,
          onEnter: (_) => setState(() => _isHovering = true),
          onExit: (_) => setState(() => _isHovering = false),
          child: Stack(
            clipBehavior: Clip.none,
            alignment: Alignment.topCenter,
            children: [
              pinWidget,
              Positioned(
                top: height,
                child: labelWidget,
              ),
            ],
          ),
        ),
      ),
    );
  }
}

class DropletPainter extends CustomPainter {
  final Color color;
  final double scale;

  DropletPainter({
    required this.color,
    this.scale = 1.0,
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
      ..cubicTo(
          w / 2, h * 0.80,
          0, h * 0.65,
          0, circleRadius
      )
      ..arcTo(Rect.fromCircle(center: center, radius: circleRadius),
          math.pi, math.pi, false)
      ..cubicTo(
          w, h * 0.65,
          w / 2, h * 0.80,
          w / 2, h
      )
      ..close();

    canvas.drawPath(path, paintFill);
  }

  @override
  bool shouldRepaint(covariant DropletPainter oldDelegate) {
    return oldDelegate.color != color || oldDelegate.scale != scale;
  }
}