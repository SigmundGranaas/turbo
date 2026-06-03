import 'dart:math' as math;

import 'package:flutter/material.dart';
import 'package:flutter_svg/flutter_svg.dart';

import '../models/weather_forecast.dart';
import '../models/weather_symbol.dart';

/// MET Norway symbol SVG (bundled from metno/weathericons, MIT) with a
/// Material fallback for `null` / unknown codes.
class WeatherSymbolIcon extends StatelessWidget {
  final WeatherSymbol? symbol;
  final double size;
  const WeatherSymbolIcon({super.key, required this.symbol, required this.size});

  @override
  Widget build(BuildContext context) {
    final s = symbol;
    final fallbackColor = Theme.of(context).colorScheme.onSurfaceVariant;
    final fallback =
        Icon(Icons.cloud_queue, size: size, color: fallbackColor);
    if (s == null || s.isFallback) return fallback;
    return SvgPicture.asset(
      s.assetPath,
      key: Key('weather-symbol-${s.code}'),
      width: size,
      height: size,
      placeholderBuilder: (_) => fallback,
    );
  }
}

/// Rotated arrow pointing in the direction the wind is *going* (downstream).
/// MET reports `fromDeg` as the bearing wind is coming from — the arrow head
/// is therefore rotated `fromDeg + 180°` from north. `null` direction renders
/// a centered dot so the row layout stays stable.
class WindArrow extends StatelessWidget {
  final double? fromDeg;
  final double size;
  final Color? color;
  const WindArrow({
    super.key,
    required this.fromDeg,
    this.size = 18,
    this.color,
  });

  @override
  Widget build(BuildContext context) {
    final c = color ?? Theme.of(context).colorScheme.primary;
    final from = fromDeg;
    if (from == null) {
      return Icon(Icons.circle, size: size * 0.4, color: c);
    }
    final goingRad = ((from + 180) % 360) * math.pi / 180;
    return Transform.rotate(
      angle: goingRad,
      child: Icon(Icons.navigation, size: size, color: c),
    );
  }
}

/// Icon variant for a wave height in metres. Calm sea → still water,
/// moderate seas → waves, big seas → a swell silhouette.
IconData waveBucketIcon(double meters) {
  if (meters < 0.5) return Icons.water;
  if (meters < 2.0) return Icons.waves;
  return Icons.tsunami;
}

/// Icon variant for a precipitation rate in mm/h. Snow gets its own ramp.
IconData precipBucketIcon(double mmPerHour, {required bool snow}) {
  if (snow) {
    if (mmPerHour < 0.2) return Icons.ac_unit_outlined;
    if (mmPerHour < 1.0) return Icons.ac_unit;
    return Icons.snowing;
  }
  if (mmPerHour < 0.2) return Icons.water_drop_outlined;
  if (mmPerHour < 2.0) return Icons.water_drop;
  return Icons.umbrella;
}

/// Icon variant for a wind speed in m/s. Pairs with [WindArrow] (which
/// keeps the rotation) — caller picks the bucket icon for badge slots
/// that don't carry direction.
IconData windBucketIcon(double ms) {
  if (ms < 5) return Icons.air;
  if (ms < 12) return Icons.air_outlined;
  return Icons.storm;
}

/// Pixel size for a `WindArrow` based on speed. Light winds render
/// smaller so a 1 m/s breeze looks meaningfully different from a gale.
double windArrowSize(double ms) {
  if (ms < 5) return 18;
  if (ms < 12) return 24;
  return 30;
}

/// Compact wind readout: arrow + "N m/s". Used on the right side of the
/// summary row and in detail-page hour rows.
class WindReadout extends StatelessWidget {
  final AtmosphericPoint point;
  final double arrowSize;
  final TextStyle? textStyle;
  const WindReadout({
    super.key,
    required this.point,
    this.arrowSize = 16,
    this.textStyle,
  });

  @override
  Widget build(BuildContext context) {
    final style =
        textStyle ?? Theme.of(context).textTheme.bodyMedium;
    return Row(
      mainAxisSize: MainAxisSize.min,
      children: [
        WindArrow(fromDeg: point.windFromDeg, size: arrowSize),
        const SizedBox(width: 4),
        Text('${point.windSpeedMs.toStringAsFixed(1)} m/s', style: style),
      ],
    );
  }
}
