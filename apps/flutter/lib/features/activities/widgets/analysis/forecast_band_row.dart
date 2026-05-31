import 'package:flutter/material.dart';

import '../../models/activity_analysis.dart';

/// Sparkline with a shaded confidence-interval band for one driver's
/// forecast. Pure `CustomPainter` — no chart-library dependency.
class ForecastBandRow extends StatelessWidget {
  final ForecastBand band;
  final Color tintColor;

  const ForecastBandRow({super.key, required this.band, required this.tintColor});

  @override
  Widget build(BuildContext context) {
    if (band.samples.isEmpty) return const SizedBox.shrink();
    return SizedBox(
      height: 56,
      child: CustomPaint(
        painter: _BandPainter(samples: band.samples, tintColor: tintColor),
        size: Size.infinite,
      ),
    );
  }
}

class _BandPainter extends CustomPainter {
  final List<ForecastSample> samples;
  final Color tintColor;

  _BandPainter({required this.samples, required this.tintColor});

  @override
  void paint(Canvas canvas, Size size) {
    if (samples.isEmpty) return;

    // Find the time + value extents. Use the CI band when present;
    // otherwise just the values.
    final firstT = samples.first.at.millisecondsSinceEpoch.toDouble();
    final lastT = samples.last.at.millisecondsSinceEpoch.toDouble();
    final tSpan = (lastT - firstT).clamp(1.0, double.infinity);

    double minV = double.infinity;
    double maxV = -double.infinity;
    for (final s in samples) {
      final lo = s.lower ?? s.value;
      final hi = s.upper ?? s.value;
      if (lo < minV) minV = lo;
      if (hi > maxV) maxV = hi;
    }
    if (maxV - minV < 1e-6) {
      // Constant series — pad so the line draws horizontally in the middle.
      maxV = minV + 1;
    }

    Offset point(double t, double v) {
      final x = ((t - firstT) / tSpan) * size.width;
      final y = size.height - ((v - minV) / (maxV - minV)) * size.height;
      return Offset(x, y);
    }

    // CI band (only when at least one sample carries a range).
    final hasBand = samples.any((s) => s.lower != null && s.upper != null);
    if (hasBand) {
      final bandPath = Path();
      var started = false;
      for (final s in samples) {
        final p = point(s.at.millisecondsSinceEpoch.toDouble(), s.upper ?? s.value);
        if (!started) {
          bandPath.moveTo(p.dx, p.dy);
          started = true;
        } else {
          bandPath.lineTo(p.dx, p.dy);
        }
      }
      for (final s in samples.reversed) {
        final p = point(s.at.millisecondsSinceEpoch.toDouble(), s.lower ?? s.value);
        bandPath.lineTo(p.dx, p.dy);
      }
      bandPath.close();
      canvas.drawPath(
        bandPath,
        Paint()..color = tintColor.withValues(alpha: 0.15),
      );
    }

    // Main line.
    final linePath = Path();
    var started = false;
    for (final s in samples) {
      final p = point(s.at.millisecondsSinceEpoch.toDouble(), s.value);
      if (!started) {
        linePath.moveTo(p.dx, p.dy);
        started = true;
      } else {
        linePath.lineTo(p.dx, p.dy);
      }
    }
    canvas.drawPath(
      linePath,
      Paint()
        ..color = tintColor
        ..style = PaintingStyle.stroke
        ..strokeWidth = 2.0
        ..strokeCap = StrokeCap.round
        ..strokeJoin = StrokeJoin.round,
    );
  }

  @override
  bool shouldRepaint(covariant _BandPainter old) =>
      old.samples != samples || old.tintColor != tintColor;
}
