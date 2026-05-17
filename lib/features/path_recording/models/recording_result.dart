import 'package:latlong2/latlong.dart';

/// What `RecordingNotifier.stop()` hands back to the UI. The UI is then
/// responsible for opening `SavePathSheet` with these values and turning
/// them into a persisted `SavedPath`.
class RecordingResult {
  final List<LatLng> points;
  final List<double?> elevations;
  final double distanceMeters;
  final int movingTimeSeconds;
  final double ascent;
  final double descent;
  final DateTime recordedAt;

  const RecordingResult({
    required this.points,
    required this.elevations,
    required this.distanceMeters,
    required this.movingTimeSeconds,
    required this.ascent,
    required this.descent,
    required this.recordedAt,
  });

  bool get isEmpty => points.length < 2;
}
