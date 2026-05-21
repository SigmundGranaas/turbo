/// Smoothed cumulative ascent / descent derived from a sequence of
/// barometric or GPS altitude samples (meters).
///
/// GPS altitude is noisy (typically ±3-5 m on phones), so naive summing of
/// raw deltas overcounts. The algorithm here applies a small sliding-window
/// mean to dampen jitter and then ignores deltas below a fixed noise floor.
class ElevationStats {
  final double ascent;
  final double descent;

  const ElevationStats({required this.ascent, required this.descent});

  static const ElevationStats zero = ElevationStats(ascent: 0, descent: 0);

  /// Window size for the sliding-mean smoother. 5 samples ≈ a few seconds
  /// of recording at 1 Hz, enough to flatten single-sample spikes without
  /// erasing legitimate short climbs.
  static const int smoothingWindow = 5;

  /// Deltas smaller than this magnitude (after smoothing) are treated as
  /// noise and dropped. 1.0 m is a common choice for consumer GPS.
  static const double noiseFloorMeters = 1.0;

  /// Computes ascent and descent from a raw altitude series.
  ///
  /// Returns [zero] if the series has fewer than two non-null entries — there
  /// is nothing meaningful to derive in that case.
  factory ElevationStats.fromSamples(List<double?> raw) {
    final filtered = <double>[];
    for (final v in raw) {
      if (v != null && v.isFinite) filtered.add(v);
    }
    if (filtered.length < 2) return zero;

    final smoothed = _slidingMean(filtered, smoothingWindow);

    double ascent = 0;
    double descent = 0;
    for (var i = 1; i < smoothed.length; i++) {
      final delta = smoothed[i] - smoothed[i - 1];
      if (delta.abs() < noiseFloorMeters) continue;
      if (delta > 0) {
        ascent += delta;
      } else {
        descent += -delta;
      }
    }
    return ElevationStats(ascent: ascent, descent: descent);
  }

  static List<double> _slidingMean(List<double> input, int window) {
    // For series shorter than the window, shrink the window rather than
    // collapsing every value to a single mean (which would erase the deltas
    // we care about).
    final effectiveWindow = window > input.length ? input.length : window;
    final half = effectiveWindow ~/ 2;
    final out = List<double>.filled(input.length, 0);
    for (var i = 0; i < input.length; i++) {
      final start = (i - half).clamp(0, input.length - 1);
      final end = (i + half + 1).clamp(0, input.length);
      var sum = 0.0;
      for (var j = start; j < end; j++) {
        sum += input[j];
      }
      out[i] = sum / (end - start);
    }
    return out;
  }
}
