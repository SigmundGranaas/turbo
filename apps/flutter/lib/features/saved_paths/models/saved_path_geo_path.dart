import 'package:turbo/core/geo/geo_path.dart';

import 'saved_path.dart';

/// Conversions between the persisted [SavedPath] and the shared [GeoPath].
///
/// `SavedPath` stores elevations as a NaN-filled `List<double>` (positional
/// alignment with points); [GeoPath] uses the canonical nullable-per-point
/// form. These extensions reconcile the two so the rest of the app only ever
/// handles `GeoPath`.
extension SavedPathToGeoPath on SavedPath {
  GeoPath toGeoPath() => GeoPath(
        points: points,
        elevations:
            elevations?.map((e) => e.isNaN ? null : e).toList(growable: false),
        distanceM: distance,
        ascentM: ascent,
        descentM: descent,
        movingTimeSeconds: movingTimeSeconds,
        recordedAt: recordedAt,
        source: GeoPathSource.saved,
      );
}

extension GeoPathToSavedPath on GeoPath {
  /// Build a persistable [SavedPath] from this path. `title` is required; other
  /// presentation fields are caller-supplied. Null elevations are written back
  /// as NaN to preserve positional alignment.
  SavedPath toSavedPath({
    required String title,
    String? description,
    String? colorHex,
    String? iconKey,
    bool smoothing = false,
    String? lineStyleKey,
  }) {
    return SavedPath(
      title: title,
      description: description,
      points: points,
      distance: distanceM,
      colorHex: colorHex,
      iconKey: iconKey,
      smoothing: smoothing,
      lineStyleKey: lineStyleKey,
      elevations:
          elevations?.map((e) => e ?? double.nan).toList(growable: false),
      recordedAt: recordedAt,
      ascent: ascentM,
      descent: descentM,
      movingTimeSeconds: movingTimeSeconds,
    );
  }
}
