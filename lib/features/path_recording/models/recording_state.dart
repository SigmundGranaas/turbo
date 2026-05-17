import 'package:latlong2/latlong.dart';

enum RecordingStatus { idle, recording, paused }

/// Immutable snapshot of an in-flight recording. The notifier emits a new
/// [RecordingState] on a throttled cadence so the HUD rebuilds at a
/// predictable rate rather than once per GPS fix.
class RecordingState {
  final RecordingStatus status;
  final List<LatLng> points;
  final List<double?> elevations;
  final double distanceMeters;
  final int movingTimeSeconds;
  final double ascent;
  final double descent;
  final DateTime? startedAt;
  final DateTime? lastFixAt;

  const RecordingState({
    required this.status,
    required this.points,
    required this.elevations,
    required this.distanceMeters,
    required this.movingTimeSeconds,
    required this.ascent,
    required this.descent,
    required this.startedAt,
    required this.lastFixAt,
  });

  static const RecordingState idle = RecordingState(
    status: RecordingStatus.idle,
    points: [],
    elevations: [],
    distanceMeters: 0,
    movingTimeSeconds: 0,
    ascent: 0,
    descent: 0,
    startedAt: null,
    lastFixAt: null,
  );

  bool get isActive =>
      status == RecordingStatus.recording || status == RecordingStatus.paused;

  RecordingState copyWith({
    RecordingStatus? status,
    List<LatLng>? points,
    List<double?>? elevations,
    double? distanceMeters,
    int? movingTimeSeconds,
    double? ascent,
    double? descent,
    DateTime? startedAt,
    DateTime? lastFixAt,
  }) {
    return RecordingState(
      status: status ?? this.status,
      points: points ?? this.points,
      elevations: elevations ?? this.elevations,
      distanceMeters: distanceMeters ?? this.distanceMeters,
      movingTimeSeconds: movingTimeSeconds ?? this.movingTimeSeconds,
      ascent: ascent ?? this.ascent,
      descent: descent ?? this.descent,
      startedAt: startedAt ?? this.startedAt,
      lastFixAt: lastFixAt ?? this.lastFixAt,
    );
  }
}
