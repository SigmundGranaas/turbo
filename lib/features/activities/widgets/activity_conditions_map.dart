import 'dart:math' as math;

import 'package:flutter/material.dart';
import 'package:flutter_map/flutter_map.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:latlong2/latlong.dart';

import 'package:turbo/features/tile_providers/api.dart' show activeTileLayersProvider;

/// Mini conditions map. Renders the activity's geometry (Point or
/// LineString) on a non-interactive map with the active tile layer
/// and overlays a few weather hints:
///   * top-left: a weather symbol icon (sun / cloud / rain / …)
///     derived from met.no's symbol_code
///   * top-right: a chip showing temperature + cloud cover
///   * bottom-right: a wind chip with a rotated arrow pointing in the
///     direction the wind is going (windFromDegrees + 180°)
///
/// Lives in the activities shell so every kind's conditions panel
/// can embed the same surface without each one re-implementing the
/// fit-to-bounds + overlay layout. Pure composition: callers pass in
/// the few primitive weather fields they care about (extracted from
/// their own typed report) — the widget knows nothing about
/// kind-specific report types.
class ActivityConditionsMap extends ConsumerWidget {
  /// Geometry to show. Single-element list for Point-kind activities;
  /// the full route for LineString-kind activities.
  final List<LatLng> points;

  /// Kind tint color — used for the polyline and the marker.
  final Color tintColor;

  /// Weather fields, extracted by the caller from its typed report.
  /// Null fields are skipped from the overlay.
  final double? airTemperatureCelsius;
  final double? cloudCoveragePct;
  final double? windFromDegrees;
  final double? windSpeedMs;
  final double? precipitationNext1hMm;
  final String? symbolCode;

  /// Container height. Tight enough to feel like a thumbnail; tall
  /// enough that the route shape reads at typical city-scale zooms.
  final double height;

  const ActivityConditionsMap({
    super.key,
    required this.points,
    required this.tintColor,
    this.airTemperatureCelsius,
    this.cloudCoveragePct,
    this.windFromDegrees,
    this.windSpeedMs,
    this.precipitationNext1hMm,
    this.symbolCode,
    this.height = 180,
  });

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    if (points.isEmpty) return const SizedBox.shrink();

    final tileLayers = ref.watch(activeTileLayersProvider);

    final cameraFit = points.length == 1
        ? CameraFit.coordinates(coordinates: [points.first], minZoom: 11, maxZoom: 14)
        : CameraFit.coordinates(coordinates: points, padding: const EdgeInsets.all(28));

    final layers = <Widget>[
      ...tileLayers,
      if (points.length >= 2)
        PolylineLayer(polylines: [
          Polyline(
            points: points,
            color: tintColor.withValues(alpha: 0.85),
            strokeWidth: 4,
            strokeCap: StrokeCap.round,
            strokeJoin: StrokeJoin.round,
          ),
        ]),
      MarkerLayer(markers: [
        // Start (or only) marker.
        Marker(
          point: points.first,
          width: 18,
          height: 18,
          child: _Dot(color: tintColor),
        ),
        // End marker for routes.
        if (points.length >= 2)
          Marker(
            point: points.last,
            width: 14,
            height: 14,
            child: _Dot(color: tintColor, hollow: true),
          ),
      ]),
    ];

    return ClipRRect(
      borderRadius: BorderRadius.circular(12),
      child: SizedBox(
        height: height,
        child: Stack(children: [
          FlutterMap(
            options: MapOptions(
              backgroundColor: Theme.of(context).colorScheme.surfaceContainerHighest,
              initialCameraFit: cameraFit,
              interactionOptions: const InteractionOptions(flags: InteractiveFlag.none),
            ),
            children: layers,
          ),
          // Top-left: weather symbol icon.
          if (symbolCode != null)
            Positioned(
              top: 8, left: 8,
              child: _Pill(child: _SymbolIcon(code: symbolCode!)),
            ),
          // Top-right: temperature + cloud chip.
          if (airTemperatureCelsius != null || cloudCoveragePct != null)
            Positioned(
              top: 8, right: 8,
              child: _Pill(
                child: Row(mainAxisSize: MainAxisSize.min, children: [
                  if (airTemperatureCelsius != null) ...[
                    const Icon(Icons.thermostat_outlined, size: 14),
                    const SizedBox(width: 2),
                    Text('${airTemperatureCelsius!.toStringAsFixed(0)}°',
                      style: const TextStyle(fontWeight: FontWeight.w600, fontSize: 12)),
                  ],
                  if (airTemperatureCelsius != null && cloudCoveragePct != null)
                    const SizedBox(width: 8),
                  if (cloudCoveragePct != null) ...[
                    const Icon(Icons.cloud_outlined, size: 14),
                    const SizedBox(width: 2),
                    Text('${cloudCoveragePct!.toStringAsFixed(0)}%',
                      style: const TextStyle(fontSize: 12)),
                  ],
                ]),
              ),
            ),
          // Bottom-right: wind chip with rotated arrow.
          if (windFromDegrees != null && windSpeedMs != null)
            Positioned(
              bottom: 8, right: 8,
              child: _Pill(
                child: Row(mainAxisSize: MainAxisSize.min, children: [
                  // Meteorological convention: windFromDegrees points at
                  // the direction the wind is coming FROM. Rotate the
                  // arrow so it points where the wind is going.
                  Transform.rotate(
                    angle: (windFromDegrees! + 180) * math.pi / 180.0,
                    child: const Icon(Icons.navigation, size: 14),
                  ),
                  const SizedBox(width: 2),
                  Text('${windSpeedMs!.toStringAsFixed(1)} m/s',
                    style: const TextStyle(fontWeight: FontWeight.w600, fontSize: 12)),
                ]),
              ),
            ),
          // Bottom-left: precipitation chip (only if it's actually raining).
          if (precipitationNext1hMm != null && precipitationNext1hMm! > 0)
            Positioned(
              bottom: 8, left: 8,
              child: _Pill(
                child: Row(mainAxisSize: MainAxisSize.min, children: [
                  const Icon(Icons.water_drop_outlined, size: 14),
                  const SizedBox(width: 2),
                  Text('${precipitationNext1hMm!.toStringAsFixed(1)} mm',
                    style: const TextStyle(fontSize: 12)),
                ]),
              ),
            ),
        ]),
      ),
    );
  }
}

class _Pill extends StatelessWidget {
  final Widget child;
  const _Pill({required this.child});

  @override
  Widget build(BuildContext context) {
    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 4),
      decoration: BoxDecoration(
        color: Theme.of(context).colorScheme.surface.withValues(alpha: 0.92),
        borderRadius: BorderRadius.circular(14),
        boxShadow: [
          BoxShadow(
            color: Colors.black.withValues(alpha: 0.12),
            blurRadius: 3,
            offset: const Offset(0, 1)),
        ],
      ),
      child: child,
    );
  }
}

class _Dot extends StatelessWidget {
  final Color color;
  final bool hollow;
  const _Dot({required this.color, this.hollow = false});

  @override
  Widget build(BuildContext context) {
    return Container(
      decoration: BoxDecoration(
        color: hollow ? Colors.white : color,
        shape: BoxShape.circle,
        border: Border.all(color: hollow ? color : Colors.white, width: 2),
        boxShadow: [
          BoxShadow(color: Colors.black.withValues(alpha: 0.25), blurRadius: 2, offset: const Offset(0, 1)),
        ],
      ),
    );
  }
}

/// Maps met.no symbol_code values (or close cousins) to a Material
/// icon. Unknown codes fall back to a neutral cloud icon.
class _SymbolIcon extends StatelessWidget {
  final String code;
  const _SymbolIcon({required this.code});

  @override
  Widget build(BuildContext context) {
    final (icon, color) = _resolve(code);
    return Icon(icon, size: 18, color: color);
  }

  static (IconData, Color) _resolve(String code) {
    final lower = code.toLowerCase();
    if (lower.contains('thunder')) return (Icons.bolt, Colors.amber.shade700);
    if (lower.contains('snow') || lower.contains('sleet')) return (Icons.ac_unit, Colors.lightBlue);
    if (lower.contains('rain') || lower.contains('showers')) return (Icons.grain, Colors.blue.shade700);
    if (lower.contains('fog') || lower.contains('mist')) return (Icons.foggy, Colors.grey);
    if (lower.contains('partlycloudy')) return (Icons.wb_cloudy, Colors.grey.shade700);
    if (lower.contains('cloudy')) return (Icons.cloud, Colors.grey.shade700);
    if (lower.contains('clearsky') || lower.contains('fair')) return (Icons.wb_sunny, Colors.amber);
    return (Icons.cloud_outlined, Colors.grey);
  }
}
