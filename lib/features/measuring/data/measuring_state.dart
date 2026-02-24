import 'package:turbo/features/measuring/models/measure_point.dart';

class MeasuringState {
  final List<MeasurePoint> points;
  final double totalDistance;
  final bool isSmoothing;
  final bool isDrawing;
  final bool showIntermediatePoints;

  const MeasuringState({
    required this.points,
    required this.totalDistance,
    required this.isSmoothing,
    required this.isDrawing,
    required this.showIntermediatePoints,
  });

  factory MeasuringState.initial() {
    return const MeasuringState(
      points: [],
      totalDistance: 0,
      isSmoothing: false,
      isDrawing: false,
      showIntermediatePoints: true,
    );
  }

  MeasuringState copyWith({
    List<MeasurePoint>? points,
    double? totalDistance,
    bool? isSmoothing,
    bool? isDrawing,
    bool? showIntermediatePoints,
  }) {
    return MeasuringState(
      points: points ?? this.points,
      totalDistance: totalDistance ?? this.totalDistance,
      isSmoothing: isSmoothing ?? this.isSmoothing,
      isDrawing: isDrawing ?? this.isDrawing,
      showIntermediatePoints:
      showIntermediatePoints ?? this.showIntermediatePoints,
    );
  }
}