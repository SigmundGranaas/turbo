import '../models/elevation_stats.dart';
import '../models/saved_path.dart';
import 'hoydedata_service.dart';

/// Result returned by [backfillElevations] so callers can decide what to
/// surface to the user.
enum ElevationBackfillStatus {
  /// The input path already had enough elevation samples; the input was
  /// returned unchanged.
  notNeeded,

  /// All missing samples were filled in successfully.
  filled,

  /// Some or all samples were filled but the network failed mid-way; the
  /// returned path carries whatever could be retrieved.
  partial,

  /// Backfill threw; the input was returned unchanged.
  failed,
}

class ElevationBackfillResult {
  final SavedPath path;
  final ElevationBackfillStatus status;
  const ElevationBackfillResult(this.path, this.status);
}

/// Threshold above which we judge a path "needs" backfill. Imports that come
/// in with most samples populated already (e.g. recorded with a barometric
/// altimeter) skip the call entirely.
const double _missingThreshold = 0.5;

/// Fills missing entries in [path.elevations] via Kartverket Høydedata and
/// recomputes [SavedPath.ascent] / [SavedPath.descent] from the smoothed
/// series.
Future<ElevationBackfillResult> backfillElevations(
  SavedPath path,
  HoydedataService service,
) async {
  final points = path.points;
  if (points.isEmpty) {
    return ElevationBackfillResult(path, ElevationBackfillStatus.notNeeded);
  }

  final existing = path.elevations;
  final missing = _missingIndices(points.length, existing);
  if (missing.isEmpty ||
      missing.length / points.length < _missingThreshold) {
    return ElevationBackfillResult(path, ElevationBackfillStatus.notNeeded);
  }

  final missingPoints = [for (final i in missing) points[i]];

  List<double?> fetched;
  try {
    fetched = await service.elevationsFor(missingPoints);
  } on HoydedataServiceException {
    return ElevationBackfillResult(path, ElevationBackfillStatus.failed);
  } catch (_) {
    return ElevationBackfillResult(path, ElevationBackfillStatus.failed);
  }

  final next = List<double?>.from(existing ?? List<double?>.filled(points.length, null));
  var filledCount = 0;
  for (var j = 0; j < missing.length; j++) {
    final v = fetched[j];
    if (v != null) {
      next[missing[j]] = v;
      filledCount++;
    }
  }

  if (filledCount == 0) {
    return ElevationBackfillResult(path, ElevationBackfillStatus.failed);
  }

  // Promote List<double?> to List<double> where every slot is non-null so
  // the persisted column is dense; otherwise carry the sparse list.
  final allFilled = next.every((e) => e != null);
  final List<double>? denseElevations =
      allFilled ? next.cast<double>() : null;

  final stats = ElevationStats.fromSamples(next);
  final updated = path.copyWith(
    elevations: denseElevations,
    ascent: stats.ascent,
    descent: stats.descent,
  );
  final status = filledCount == missing.length
      ? ElevationBackfillStatus.filled
      : ElevationBackfillStatus.partial;
  return ElevationBackfillResult(updated, status);
}

List<int> _missingIndices(int total, List<double>? existing) {
  if (existing == null) return [for (var i = 0; i < total; i++) i];
  if (existing.length < total) {
    // Trailing missing entries.
    return [
      for (var i = 0; i < total; i++)
        if (i >= existing.length) i,
    ];
  }
  return [
    for (var i = 0; i < total; i++)
      if (existing[i].isNaN) i,
  ];
}
