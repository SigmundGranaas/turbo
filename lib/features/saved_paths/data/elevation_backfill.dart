import 'package:flutter_riverpod/flutter_riverpod.dart';

import 'package:turbo/core/api/kartverket_hoydedata_client.dart';
import '../models/elevation_stats.dart';
import '../models/saved_path.dart';
import 'hoydedata_service.dart';

/// Result returned by [ElevationBackfillService.backfill] so callers can
/// decide what to surface to the user.
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

/// Imports come in with widely varying elevation coverage: GPX files from
/// barometric watches are dense, web-export tracks are usually empty.
/// This service decides whether the gap is wide enough to justify a
/// Kartverket round-trip and merges the results back into the path.
///
/// Stateless wrapper for consistency with the rest of `data/` (which is
/// classes, not free functions). The Riverpod provider hands out a
/// shared instance keyed to the shared Hoydedata client.
class ElevationBackfillService {
  /// Threshold above which we judge a path "needs" backfill. Imports
  /// that come in with most samples populated already (e.g. recorded
  /// with a barometric altimeter) skip the call entirely.
  static const double _missingThreshold = 0.5;

  final HoydedataService _service;

  ElevationBackfillService({required HoydedataService service})
      : _service = service;

  /// Fills missing entries in [path.elevations] via Kartverket Høydedata
  /// and recomputes [SavedPath.ascent] / [SavedPath.descent] from the
  /// smoothed series.
  Future<ElevationBackfillResult> backfill(SavedPath path) async {
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
      fetched = await _service.elevationsFor(missingPoints);
    } on HoydedataServiceException {
      return ElevationBackfillResult(path, ElevationBackfillStatus.failed);
    } catch (_) {
      return ElevationBackfillResult(path, ElevationBackfillStatus.failed);
    }

    final next = List<double?>.from(
        existing ?? List<double?>.filled(points.length, null));
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

    // Promote List<double?> to List<double> where every slot is non-null
    // so the persisted column is dense; otherwise carry the sparse list.
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

  static List<int> _missingIndices(int total, List<double>? existing) {
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
}

/// Top-level helper retained for callers (and tests) that take a
/// [HoydedataService] directly without going through Riverpod.
Future<ElevationBackfillResult> backfillElevations(
  SavedPath path,
  HoydedataService service,
) =>
    ElevationBackfillService(service: service).backfill(path);

/// Shared provider. Backed by the core/api Hoydedata client so the same
/// HTTP plumbing serves search elevation enrichment and saved-paths
/// backfill — and tests can override at one point.
final hoydedataServiceProvider = Provider<HoydedataService>((ref) {
  return HoydedataService(client: ref.watch(kartverketHoydedataClientProvider));
});

final elevationBackfillServiceProvider =
    Provider<ElevationBackfillService>((ref) {
  return ElevationBackfillService(
    service: ref.watch(hoydedataServiceProvider),
  );
});
