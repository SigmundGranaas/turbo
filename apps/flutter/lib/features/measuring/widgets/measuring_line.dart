import 'package:flutter/material.dart';
import 'package:flutter_map/flutter_map.dart';
import 'package:latlong2/latlong.dart';

class MeasurePolyline extends StatelessWidget {
  final List<LatLng> points;

  const MeasurePolyline({super.key, required this.points});

  @override
  Widget build(BuildContext context) {
    if (points.length < 2) {
      return const SizedBox.shrink();
    }
    return PolylineLayer(
      polylines: [
        Polyline(
          points: points,
          strokeWidth: 3,
          color: Theme.of(context).primaryColor.withAlpha(200),
          strokeCap: StrokeCap.round,
          strokeJoin: StrokeJoin.round,
        ),
      ],
    );
  }
}