import 'package:flutter/material.dart';
import 'package:flutter_map/flutter_map.dart';

import 'measure_point.dart';

class MeasureMarkers extends StatelessWidget {
  final List<MeasurePoint> points;

  const MeasureMarkers({super.key, required this.points});

  @override
  Widget build(BuildContext context) {
    return MarkerLayer(
      markers: points.map((point) {
        return Marker(
          point: point.point,
          child: TweenAnimationBuilder<double>(
            duration: const Duration(milliseconds: 300),
            tween: Tween(begin: 0.0, end: 1.0),
            curve: Curves.elasticOut,
            builder: (context, value, child) {
              return Transform.scale(
                scale: value,
                child: Icon(
                  point.type.icon,
                  color: Theme.of(context).primaryColor,
                  size: 16,
                ),
              );
            },
          ),
        );
      }).toList(),
    );
  }
}