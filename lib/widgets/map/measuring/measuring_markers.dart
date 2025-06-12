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
          width: point.type.size,
          height: point.type.size,
          point: point.point,
          child: TweenAnimationBuilder<double>(
            duration: const Duration(milliseconds: 300),
            tween: Tween(begin: 0.0, end: 1.0),
            curve: Curves.elasticOut,
            builder: (context, value, child) {
              return Transform.scale(
                scale: value,
                child: Container(
                  decoration: BoxDecoration(
                    shape: BoxShape.circle,
                    color: point.type.getColor(context),
                    border: Border.all(
                      color: Theme.of(context).colorScheme.surface,
                      width: 2.0,
                    ),
                    boxShadow: const [
                      BoxShadow(
                        color: Colors.black26,
                        blurRadius: 4,
                        offset: Offset(0, 2),
                      ),
                    ],
                  ),
                ),
              );
            },
          ),
        );
      }).toList(),
    );
  }
}