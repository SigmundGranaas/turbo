import 'package:flutter/material.dart';
import 'package:flutter_map/flutter_map.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:latlong2/latlong.dart';

import '../providers/ntb_providers.dart';
import '../util/route_reveal.dart';

/// Presentation layer that "draws the hike in" when a trip marker is selected.
/// Watches [ntbSelectedRouteProvider]; on a new selection it fits the camera to
/// the route and runs an [AnimationController] whose value drives
/// [RouteReveal.revealPolyline], so the polyline is rebuilt every frame and
/// appears to grow from start to finish.
class NtbRouteLayer extends ConsumerStatefulWidget {
  final MapController mapController;
  final bool visible;

  const NtbRouteLayer({
    super.key,
    required this.mapController,
    this.visible = true,
  });

  @override
  ConsumerState<NtbRouteLayer> createState() => _NtbRouteLayerState();
}

class _NtbRouteLayerState extends ConsumerState<NtbRouteLayer>
    with SingleTickerProviderStateMixin {
  late final AnimationController _controller;
  late final Animation<double> _curve;

  List<LatLng> _points = const [];
  List<double> _cumulative = const [];
  int _animatedToken = -1;

  static const Color _routeColor = Color(0xFF2E7D32);

  @override
  void initState() {
    super.initState();
    _controller = AnimationController(
      vsync: this,
      duration: const Duration(milliseconds: 1600),
    );
    _curve = CurvedAnimation(parent: _controller, curve: Curves.easeInOut);
  }

  @override
  void dispose() {
    _controller.dispose();
    super.dispose();
  }

  void _startReveal(List<LatLng> points) {
    _points = points;
    _cumulative = RouteReveal.cumulativeDistances(points);
    _fitCamera(points);
    _controller
      ..reset()
      ..forward();
  }

  void _clear() {
    _points = const [];
    _cumulative = const [];
    _controller.reset();
  }

  void _fitCamera(List<LatLng> points) {
    if (points.length < 2) return;
    try {
      widget.mapController.fitCamera(
        CameraFit.bounds(
          bounds: LatLngBounds.fromPoints(points),
          padding: const EdgeInsets.fromLTRB(48, 48, 48, 280),
        ),
      );
    } catch (_) {
      // Camera not ready yet — the reveal still runs, just without re-framing.
    }
  }

  @override
  Widget build(BuildContext context) {
    final selection = ref.watch(ntbSelectedRouteProvider);

    // React to selection changes outside of build (camera + animation are
    // side effects). A token bump means a fresh selection to present.
    if (!widget.visible || selection == null) {
      if (_points.isNotEmpty) {
        _animatedToken = -1;
        WidgetsBinding.instance.addPostFrameCallback((_) {
          if (mounted) setState(_clear);
        });
      }
    } else {
      final route = selection.route;
      if (route != null &&
          route.hasGeometry &&
          selection.token != _animatedToken) {
        _animatedToken = selection.token;
        final points = route.points;
        WidgetsBinding.instance.addPostFrameCallback((_) {
          if (mounted) setState(() => _startReveal(points));
        });
      }
    }

    if (!widget.visible || _points.length < 2) return const SizedBox.shrink();

    return AnimatedBuilder(
      animation: _curve,
      builder: (context, _) {
        final revealed = RouteReveal.revealPolyline(
          _points,
          _curve.value,
          cumulative: _cumulative,
        );
        if (revealed.length < 2) return const SizedBox.shrink();
        final tip = revealed.last;
        return Stack(
          children: [
            PolylineLayer(
              polylines: [
                Polyline(
                  points: revealed,
                  color: _routeColor,
                  strokeWidth: 5,
                  borderColor: Colors.white,
                  borderStrokeWidth: 2,
                ),
              ],
            ),
            MarkerLayer(
              markers: [
                Marker(
                  width: 18,
                  height: 18,
                  point: tip,
                  child: DecoratedBox(
                    decoration: BoxDecoration(
                      color: _routeColor,
                      shape: BoxShape.circle,
                      border: Border.all(color: Colors.white, width: 2),
                    ),
                  ),
                ),
              ],
            ),
          ],
        );
      },
    );
  }
}
