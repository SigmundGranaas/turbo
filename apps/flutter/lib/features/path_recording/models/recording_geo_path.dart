import 'package:turbo/core/geo/geo_path.dart';

import 'recording_result.dart';

/// Bridge from a finished recording to the shared [GeoPath]. Recording already
/// stores per-sample nullable elevations, so this is a direct mapping.
extension RecordingResultGeoPath on RecordingResult {
  GeoPath toGeoPath() => GeoPath(
        points: points,
        elevations: elevations,
        distanceM: distanceMeters,
        ascentM: ascent,
        descentM: descent,
        movingTimeSeconds: movingTimeSeconds,
        recordedAt: recordedAt,
        source: GeoPathSource.recording,
      );
}
