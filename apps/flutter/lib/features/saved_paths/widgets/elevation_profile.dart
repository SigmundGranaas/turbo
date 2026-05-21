import 'package:flutter/material.dart';
import 'package:turbo/features/settings/api.dart';

/// Sparkline of altitude vs cumulative distance for a saved path.
///
/// Renders a filled curve in the path's color (or the theme accent) plus
/// min/max altitude labels. Pure CustomPainter — no extra dependency. Null
/// entries in [elevations] are interpolated linearly between neighbours so
/// gaps don't punch holes in the line.
class ElevationProfile extends StatelessWidget {
  final List<double?> elevations;
  final DistanceUnit unit;
  final Color? lineColor;
  final double height;

  const ElevationProfile({
    super.key,
    required this.elevations,
    this.unit = DistanceUnit.metric,
    this.lineColor,
    this.height = 96,
  });

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final color = lineColor ?? theme.colorScheme.primary;

    final cleaned = _interpolateNulls(elevations);
    if (cleaned.length < 2) {
      return SizedBox(
        height: height,
        child: Center(
          child: Text(
            'No elevation data',
            style: theme.textTheme.bodySmall?.copyWith(
              color: theme.colorScheme.onSurfaceVariant,
            ),
          ),
        ),
      );
    }

    var minV = cleaned.first;
    var maxV = cleaned.first;
    for (final v in cleaned) {
      if (v < minV) minV = v;
      if (v > maxV) maxV = v;
    }

    final textStyle = theme.textTheme.bodySmall?.copyWith(
      color: theme.colorScheme.onSurfaceVariant,
    );

    return SizedBox(
      height: height,
      child: Stack(
        children: [
          Positioned.fill(
            child: CustomPaint(
              painter: _ProfilePainter(
                values: cleaned,
                color: color,
                fillColor: color.withValues(alpha: 0.18),
                minValue: minV,
                maxValue: maxV,
              ),
            ),
          ),
          Positioned(
            top: 4,
            right: 8,
            child: Text(_formatElevation(maxV), style: textStyle),
          ),
          Positioned(
            bottom: 4,
            right: 8,
            child: Text(_formatElevation(minV), style: textStyle),
          ),
        ],
      ),
    );
  }

  String _formatElevation(double meters) {
    if (unit == DistanceUnit.imperial) {
      final feet = meters / 0.3048;
      return '${feet.round()} ft';
    }
    return '${meters.round()} m';
  }

  List<double> _interpolateNulls(List<double?> input) {
    final out = <double>[];
    double? lastValid;
    final pendingNullIndices = <int>[];

    for (final v in input) {
      if (v != null && v.isFinite) {
        if (pendingNullIndices.isNotEmpty) {
          // Linearly interpolate the gap between the previous valid value and
          // this one. If there is no previous valid value, we'll backfill
          // after we know the first one.
          if (lastValid != null) {
            final span = pendingNullIndices.length + 1;
            for (var i = 0; i < pendingNullIndices.length; i++) {
              final t = (i + 1) / span;
              out.add(lastValid + (v - lastValid) * t);
            }
          } else {
            // Backfill leading nulls with the first valid value.
            for (var _ in pendingNullIndices) {
              out.add(v);
            }
          }
          pendingNullIndices.clear();
        }
        out.add(v);
        lastValid = v;
      } else {
        pendingNullIndices.add(out.length);
      }
    }
    // Trailing nulls take the last valid value (or are dropped if all null).
    if (pendingNullIndices.isNotEmpty && lastValid != null) {
      for (var _ in pendingNullIndices) {
        out.add(lastValid);
      }
    }
    return out;
  }
}

class _ProfilePainter extends CustomPainter {
  final List<double> values;
  final Color color;
  final Color fillColor;
  final double minValue;
  final double maxValue;

  _ProfilePainter({
    required this.values,
    required this.color,
    required this.fillColor,
    required this.minValue,
    required this.maxValue,
  });

  @override
  void paint(Canvas canvas, Size size) {
    if (values.length < 2) return;
    final range = (maxValue - minValue).abs() < 0.5 ? 1.0 : (maxValue - minValue);

    final path = Path();
    final fill = Path();
    final dx = size.width / (values.length - 1);

    for (var i = 0; i < values.length; i++) {
      final x = i * dx;
      final normalized = (values[i] - minValue) / range;
      final y = size.height - normalized * size.height;
      if (i == 0) {
        path.moveTo(x, y);
        fill.moveTo(x, size.height);
        fill.lineTo(x, y);
      } else {
        path.lineTo(x, y);
        fill.lineTo(x, y);
      }
    }
    fill.lineTo(size.width, size.height);
    fill.close();

    final fillPaint = Paint()..color = fillColor;
    canvas.drawPath(fill, fillPaint);

    final strokePaint = Paint()
      ..color = color
      ..style = PaintingStyle.stroke
      ..strokeWidth = 2
      ..strokeJoin = StrokeJoin.round;
    canvas.drawPath(path, strokePaint);
  }

  @override
  bool shouldRepaint(covariant _ProfilePainter old) =>
      old.values != values ||
      old.color != color ||
      old.minValue != minValue ||
      old.maxValue != maxValue;
}
