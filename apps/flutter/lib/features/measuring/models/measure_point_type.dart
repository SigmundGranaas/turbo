import 'package:flutter/material.dart';

enum MeasurePointType {
  start,
  middle,
  end;

  double get size {
    switch (this) {
      case MeasurePointType.start:
      case MeasurePointType.end:
        return 18.0;
      case MeasurePointType.middle:
        return 12.0;
    }
  }

  Color getColor(BuildContext context) {
    final colorScheme = Theme.of(context).colorScheme;
    switch (this) {
      case MeasurePointType.start:
        return colorScheme.primary;
      case MeasurePointType.middle:
        return colorScheme.secondary;
      case MeasurePointType.end:
        return colorScheme.tertiary;
    }
  }
}