import 'package:flutter/material.dart';
import 'package:flutter_map/flutter_map.dart';

import 'measure_point.dart';

class MeasurePolyline extends StatelessWidget {
  final List<MeasurePoint> points;

  const MeasurePolyline({super.key, required this.points});

  @override
  Widget build(BuildContext context) {
    return PolylineLayer(
      polylines: [
        Polyline(
          points: points.map((p) => p.point).toList(),
          strokeWidth: 3,
          color: Theme.of(context).primaryColor.withOpacity(0.8),
          strokeCap: StrokeCap.round,
          strokeJoin: StrokeJoin.round,
        ),
      ],
    );
  }
}