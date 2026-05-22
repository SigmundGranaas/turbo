import 'package:turbo/features/measuring/models/measure_point.dart';

class MeasuringState {
  final List<MeasurePoint> points;
  final double totalDistance;
  final bool isDrawing;

  const MeasuringState({
    required this.points,
    required this.totalDistance,
    required this.isDrawing,
  });

  factory MeasuringState.initial() {
    return const MeasuringState(
      points: [],
      totalDistance: 0,
      isDrawing: false,
    );
  }

  MeasuringState copyWith({
    List<MeasurePoint>? points,
    double? totalDistance,
    bool? isDrawing,
  }) {
    return MeasuringState(
      points: points ?? this.points,
      totalDistance: totalDistance ?? this.totalDistance,
      isDrawing: isDrawing ?? this.isDrawing,
    );
  }
}