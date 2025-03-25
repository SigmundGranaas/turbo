import 'package:flutter/material.dart';

enum MeasurePointType {
  start,
  middle,
  end;

  IconData get icon {
    switch (this) {
      case MeasurePointType.start:
      // A clear "start" indicator that's different but related to measurement
        return Icons.fiber_manual_record;
      case MeasurePointType.middle:
      // A smaller waypoint indicator
        return Icons.circle;
      case MeasurePointType.end:
      // Current position/endpoint indicator
        return Icons.lens;
    }
  }

  double get size {
    switch (this) {
      case MeasurePointType.start:
        return 24.0;
      case MeasurePointType.middle:
        return 16.0;
      case MeasurePointType.end:
        return 20.0;
    }
  }

  Color getColor(BuildContext context) {
    final primary = Theme.of(context).primaryColor;
    switch (this) {
      case MeasurePointType.start:
        return primary;
      case MeasurePointType.middle:
        return primary.withValues(alpha: 0.7);
      case MeasurePointType.end:
        return primary;
    }
  }
}