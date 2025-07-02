import 'package:turbo/features/measuring/models/measure_point.dart';

class MeasuringState {
  final List<MeasurePoint> points;
  final double totalDistance;
  final bool isSmoothing;
  final bool isDrawing;
  final double drawSensitivity;
  final bool showIntermediatePoints;

  const MeasuringState({
    required this.points,
    required this.totalDistance,
    required this.isSmoothing,
    required this.isDrawing,
    required this.drawSensitivity,
    required this.showIntermediatePoints,
  });

  factory MeasuringState.initial() {
    return const MeasuringState(
      points: [],
      totalDistance: 0,
      isSmoothing: false,
      isDrawing: false,
      drawSensitivity: 15.0,
      showIntermediatePoints: true,
    );
  }

  MeasuringState copyWith({
    List<MeasurePoint>? points,
    double? totalDistance,
    bool? isSmoothing,
    bool? isDrawing,
    double? drawSensitivity,
    bool? showIntermediatePoints,
  }) {
    return MeasuringState(
      points: points ?? this.points,
      totalDistance: totalDistance ?? this.totalDistance,
      isSmoothing: isSmoothing ?? this.isSmoothing,
      isDrawing: isDrawing ?? this.isDrawing,
      drawSensitivity: drawSensitivity ?? this.drawSensitivity,
      showIntermediatePoints:
      showIntermediatePoints ?? this.showIntermediatePoints,
    );
  }
}