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

/// 8-point compass label from a "from" bearing.
String compassDir(double deg) {
  const dirs = ['N', 'NE', 'E', 'SE', 'S', 'SW', 'W', 'NW'];
  final idx = ((deg % 360) / 45).round() % 8;
  return dirs[idx];
}

String formatWind(AtmosphericPoint p) {
  final dir = p.windFromDeg == null ? '' : ' ${compassDir(p.windFromDeg!)}';
  return '${p.windSpeedMs.toStringAsFixed(1)} m/s$dir';
}

/// Short, single-line headline for the now-cast — used by the in-sheet summary
/// row. Wind always shown; precipitation appended when measurable.
String nowcastSummary(AtmosphericPoint p) {
  final wind = formatWind(p);
  final precip = p.precipitation1hMm;
  if (precip != null && precip > 0) {
    return '$wind  ·  ${precip.toStringAsFixed(1)} mm/h';
  }
  return wind;
}
