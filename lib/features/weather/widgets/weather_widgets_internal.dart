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

/// Small icon + label + value triple used across the weather UI.
class WeatherChip extends StatelessWidget {
  final IconData icon;
  final String label;
  final String value;
  const WeatherChip({
    super.key,
    required this.icon,
    required this.label,
    required this.value,
  });

  @override
  Widget build(BuildContext context) {
    final textTheme = Theme.of(context).textTheme;
    final colorScheme = Theme.of(context).colorScheme;
    return Row(
      mainAxisSize: MainAxisSize.min,
      children: [
        Icon(icon, size: 14, color: colorScheme.onSurfaceVariant),
        const SizedBox(width: 4),
        Text(label,
            style: textTheme.bodySmall
                ?.copyWith(color: colorScheme.onSurfaceVariant)),
        const SizedBox(width: 4),
        Text(value, style: textTheme.bodyMedium),
      ],
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
