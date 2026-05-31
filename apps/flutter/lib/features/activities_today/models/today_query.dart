import 'package:latlong2/latlong.dart';

/// Value-typed query for the Today screen. Used as a Riverpod family
/// key — equality is structural so changing any field invalidates the
/// cached recommendation list.
class TodayQuery {
  final LatLng location;
  final DateTime at;
  final Set<String>? kinds;
  final double radiusKm;

  const TodayQuery({
    required this.location,
    required this.at,
    this.kinds,
    this.radiusKm = 25.0,
  });

  @override
  bool operator ==(Object other) {
    if (identical(this, other)) return true;
    if (other is! TodayQuery) return false;
    return location.latitude == other.location.latitude &&
        location.longitude == other.location.longitude &&
        at.isAtSameMomentAs(other.at) &&
        radiusKm == other.radiusKm &&
        _setEq(kinds, other.kinds);
  }

  @override
  int get hashCode => Object.hash(
        location.latitude.toStringAsFixed(4),
        location.longitude.toStringAsFixed(4),
        at.toUtc().millisecondsSinceEpoch,
        radiusKm,
        kinds == null ? 0 : Object.hashAllUnordered(kinds!),
      );

  static bool _setEq(Set<String>? a, Set<String>? b) {
    if (identical(a, b)) return true;
    if (a == null || b == null) return false;
    if (a.length != b.length) return false;
    for (final v in a) {
      if (!b.contains(v)) return false;
    }
    return true;
  }
}
