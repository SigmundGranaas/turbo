import 'package:latlong2/latlong.dart';

/// Pure geometry helpers for the "draw the hike in" presentation. Kept free of
/// any Flutter/widget dependency so the reveal maths can be unit-tested in
/// isolation from the animated layer that drives it.
class RouteReveal {
  RouteReveal._();

  static final Distance _distance = const Distance();

  /// Cumulative great-circle distance (metres) along [points]. The first entry
  /// is always `0`; the last entry is the total route length. Returns an empty
  /// list for fewer than two points.
  static List<double> cumulativeDistances(List<LatLng> points) {
    if (points.length < 2) return const [];
    final out = List<double>.filled(points.length, 0);
    var acc = 0.0;
    for (var i = 1; i < points.length; i++) {
      acc += _distance(points[i - 1], points[i]);
      out[i] = acc;
    }
    return out;
  }

  /// The total route length in metres (0 for degenerate input).
  static double totalLength(List<LatLng> points, {List<double>? cumulative}) {
    final c = cumulative ?? cumulativeDistances(points);
    return c.isEmpty ? 0 : c.last;
  }

  /// Returns the leading portion of [points] up to fraction [t] (clamped to
  /// `0..1`) of the total length, with the final vertex linearly interpolated
  /// so the polyline appears to grow continuously rather than snapping vertex
  /// to vertex. This is what the route layer rebuilds every animation frame.
  static List<LatLng> revealPolyline(
    List<LatLng> points,
    double t, {
    List<double>? cumulative,
  }) {
    if (points.isEmpty) return const [];
    if (points.length == 1) return [points.first];
    final c = cumulative ?? cumulativeDistances(points);
    final total = c.last;
    final f = t.clamp(0.0, 1.0);
    if (f <= 0 || total == 0) return [points.first];
    if (f >= 1) return List<LatLng>.from(points);

    final target = total * f;
    // Walk segments until the cumulative distance reaches the target.
    for (var i = 1; i < points.length; i++) {
      if (c[i] >= target) {
        final segStart = c[i - 1];
        final segLen = c[i] - segStart;
        final localT = segLen == 0 ? 0.0 : (target - segStart) / segLen;
        final tip = _lerp(points[i - 1], points[i], localT);
        return [...points.sublist(0, i), tip];
      }
    }
    return List<LatLng>.from(points);
  }

  /// The point at fraction [t] along the route — i.e. the moving "head" of the
  /// reveal, useful for a tip marker.
  static LatLng? pointAt(
    List<LatLng> points,
    double t, {
    List<double>? cumulative,
  }) {
    final revealed = revealPolyline(points, t, cumulative: cumulative);
    return revealed.isEmpty ? null : revealed.last;
  }

  static LatLng _lerp(LatLng a, LatLng b, double t) => LatLng(
        a.latitude + (b.latitude - a.latitude) * t,
        a.longitude + (b.longitude - a.longitude) * t,
      );
}
