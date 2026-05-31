import 'package:flutter/material.dart';
import 'package:flutter_map/flutter_map.dart';
import 'package:latlong2/latlong.dart';
import 'package:turbo/app/shadows.dart';

/// Renders the route's waypoints: a green start, a flagged end, and
/// numbered intermediate stops. Tapping a marker invokes [onTapIndex]
/// (used to remove that stop). Mirrors the measuring feature's marker
/// styling (circle + surface border + map-overlay shadow).
class RouteWaypointMarkers extends StatelessWidget {
  final List<LatLng> waypoints;
  final void Function(int index)? onTapIndex;

  const RouteWaypointMarkers({
    super.key,
    required this.waypoints,
    this.onTapIndex,
  });

  @override
  Widget build(BuildContext context) {
    final scheme = Theme.of(context).colorScheme;
    return MarkerLayer(
      markers: [
        for (var i = 0; i < waypoints.length; i++)
          Marker(
            width: 30,
            height: 30,
            point: waypoints[i],
            alignment: Alignment.center,
            child: _WaypointDot(
              color: _colorFor(i, scheme),
              onTap: onTapIndex == null ? null : () => onTapIndex!(i),
              child: _childFor(i),
            ),
          ),
      ],
    );
  }

  bool _isStart(int i) => i == 0;
  bool _isEnd(int i) => i == waypoints.length - 1 && waypoints.length > 1;

  Color _colorFor(int i, ColorScheme scheme) {
    if (_isStart(i)) return const Color(0xFF2E7D32); // green start
    if (_isEnd(i)) return scheme.error; // end
    return scheme.primary; // via
  }

  Widget? _childFor(int i) {
    if (_isStart(i)) {
      return const Icon(Icons.trip_origin, size: 14, color: Colors.white);
    }
    if (_isEnd(i)) {
      return const Icon(Icons.flag, size: 16, color: Colors.white);
    }
    return Text(
      '$i',
      style: const TextStyle(
        color: Colors.white,
        fontSize: 13,
        fontWeight: FontWeight.w700,
      ),
    );
  }
}

class _WaypointDot extends StatelessWidget {
  final Color color;
  final Widget? child;
  final VoidCallback? onTap;

  const _WaypointDot({required this.color, this.child, this.onTap});

  @override
  Widget build(BuildContext context) {
    return GestureDetector(
      onTap: onTap,
      child: TweenAnimationBuilder<double>(
        duration: const Duration(milliseconds: 280),
        tween: Tween(begin: 0.0, end: 1.0),
        curve: Curves.elasticOut,
        builder: (context, value, _) => Transform.scale(
          scale: value,
          child: Container(
            alignment: Alignment.center,
            decoration: BoxDecoration(
              shape: BoxShape.circle,
              color: color,
              border: Border.all(
                color: Theme.of(context).colorScheme.surface,
                width: 2.5,
              ),
              boxShadow: AppShadows.mapOverlay,
            ),
            child: child,
          ),
        ),
      ),
    );
  }
}
