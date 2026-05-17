import '../models/saved_path.dart';

/// Aggregated trip statistics derived from a collection of [SavedPath]s.
/// Pure data, no Riverpod — the UI consumes this through a derived provider.
class TripStats {
  final int totalPaths;
  final int recordedPaths;
  final double totalDistanceMeters;
  final double totalAscentMeters;
  final int totalMovingTimeSeconds;
  final double longestPathMeters;
  final int distinctRecordingDays;

  const TripStats({
    required this.totalPaths,
    required this.recordedPaths,
    required this.totalDistanceMeters,
    required this.totalAscentMeters,
    required this.totalMovingTimeSeconds,
    required this.longestPathMeters,
    required this.distinctRecordingDays,
  });

  static const TripStats empty = TripStats(
    totalPaths: 0,
    recordedPaths: 0,
    totalDistanceMeters: 0,
    totalAscentMeters: 0,
    totalMovingTimeSeconds: 0,
    longestPathMeters: 0,
    distinctRecordingDays: 0,
  );

  factory TripStats.from(Iterable<SavedPath> paths) {
    if (paths.isEmpty) return empty;
    var distance = 0.0;
    var ascent = 0.0;
    var movingSeconds = 0;
    var longest = 0.0;
    var recorded = 0;
    final days = <String>{};
    for (final p in paths) {
      distance += p.distance;
      if (p.distance > longest) longest = p.distance;
      if (p.ascent != null) ascent += p.ascent!;
      if (p.movingTimeSeconds != null) movingSeconds += p.movingTimeSeconds!;
      if (p.recordedAt != null) {
        recorded++;
        final d = p.recordedAt!;
        days.add('${d.year}-${d.month}-${d.day}');
      }
    }
    return TripStats(
      totalPaths: paths.length,
      recordedPaths: recorded,
      totalDistanceMeters: distance,
      totalAscentMeters: ascent,
      totalMovingTimeSeconds: movingSeconds,
      longestPathMeters: longest,
      distinctRecordingDays: days.length,
    );
  }
}
