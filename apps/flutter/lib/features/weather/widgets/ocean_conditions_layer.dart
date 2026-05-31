import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_map/flutter_map.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import 'package:turbo/app/l10n/app_localizations.dart';
import 'package:turbo/app/tokens.dart';
import 'package:turbo/features/markers/api.dart' as markers show Marker;

import '../data/ocean_conditions_notifier.dart';
import 'weather_detail_sheet.dart';
import 'weather_widgets_internal.dart' show WindArrow;

/// Map overlay that visualises live ocean conditions across the viewport.
///
/// When [visible], it samples MET Norway's `oceanforecast/2.0` endpoint over a
/// grid covering the current bounds (via [oceanConditionsProvider]) and draws a
/// wave-height-coloured chip with a direction arrow at each sea point. Tapping
/// a chip opens the full weather/ocean forecast sheet for that coordinate.
///
/// Mirrors the lifecycle of [VectorDataLayer]: it listens to the map controller
/// for pan/zoom-end events and refreshes for the new viewport, gated by
/// [minZoom] so a world-spanning bounds doesn't sample mostly-empty cells.
class OceanConditionsLayer extends ConsumerStatefulWidget {
  final MapController mapController;
  final bool visible;

  /// Below this zoom the viewport is too large for a coarse grid to be
  /// meaningful — most cells land on shore or far offshore.
  final double minZoom;

  const OceanConditionsLayer({
    super.key,
    required this.mapController,
    this.visible = true,
    this.minZoom = 5,
  });

  @override
  ConsumerState<OceanConditionsLayer> createState() =>
      _OceanConditionsLayerState();
}

class _OceanConditionsLayerState extends ConsumerState<OceanConditionsLayer> {
  StreamSubscription<MapEvent>? _eventSub;
  bool _bootstrapped = false;

  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addPostFrameCallback((_) {
      _eventSub = widget.mapController.mapEventStream.listen((event) {
        if (event is MapEventMoveEnd ||
            event is MapEventRotateEnd ||
            event is MapEventFlingAnimationEnd ||
            event is MapEventDoubleTapZoomEnd ||
            event is MapEventScrollWheelZoom) {
          _refresh();
        }
      });
      _refresh();
    });
  }

  @override
  void didUpdateWidget(OceanConditionsLayer oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (widget.visible && !oldWidget.visible) {
      _refresh();
    } else if (!widget.visible && oldWidget.visible) {
      ref.read(oceanConditionsProvider.notifier).clear();
    }
  }

  @override
  void dispose() {
    _eventSub?.cancel();
    super.dispose();
  }

  void _refresh() {
    if (!mounted || !widget.visible) return;
    final camera = widget.mapController.camera;
    if (camera.zoom < widget.minZoom) {
      _bootstrapped = true;
      return;
    }
    ref
        .read(oceanConditionsProvider.notifier)
        .requestBounds(camera.visibleBounds);
    _bootstrapped = true;
  }

  @override
  Widget build(BuildContext context) {
    if (!widget.visible) return const SizedBox.shrink();
    if (!_bootstrapped) {
      // Defer the first read until our listener has fired so the map tree
      // doesn't flicker.
      return const SizedBox.shrink();
    }

    final async = ref.watch(oceanConditionsProvider);
    final samples = async.asData?.value ?? const <OceanGridSample>[];
    if (samples.isEmpty) return const SizedBox.shrink();

    return MarkerLayer(
      markers: [
        for (final sample in samples)
          Marker(
            point: sample.position,
            width: 76,
            height: 30,
            child: _WaveChip(
              sample: sample,
              onTap: () => _openForecast(context, sample),
            ),
          ),
      ],
    );
  }

  void _openForecast(BuildContext context, OceanGridSample sample) {
    final title = context.l10n.oceanConditionsTitle;
    showWeatherDetailSheet(
      context,
      markers.Marker(title: title, position: sample.position),
    );
  }
}

/// A wave-height-coloured pill: direction arrow + "N.N m". The fill colour
/// follows a calm→high sea-state ramp so the field of chips reads as a
/// heat-map at a glance.
class _WaveChip extends StatelessWidget {
  final OceanGridSample sample;
  final VoidCallback onTap;

  const _WaveChip({required this.sample, required this.onTap});

  @override
  Widget build(BuildContext context) {
    final heightM = sample.point.waveHeightM ?? 0;
    final fill = waveStateColor(heightM);
    final onFill = ThemeData.estimateBrightnessForColor(fill) == Brightness.dark
        ? Colors.white
        : Colors.black87;

    return GestureDetector(
      onTap: onTap,
      child: DecoratedBox(
        decoration: BoxDecoration(
          color: fill,
          borderRadius: BorderRadius.circular(AppRadius.pill),
          boxShadow: const [
            BoxShadow(
              color: Color(0x33000000),
              blurRadius: 3,
              offset: Offset(0, 1),
            ),
          ],
        ),
        child: Padding(
          padding: const EdgeInsets.symmetric(horizontal: 6),
          child: Row(
            mainAxisSize: MainAxisSize.min,
            mainAxisAlignment: MainAxisAlignment.center,
            children: [
              WindArrow(
                fromDeg: sample.point.waveFromDeg,
                size: 16,
                color: onFill,
              ),
              const SizedBox(width: 4),
              Text(
                '${heightM.toStringAsFixed(1)} m',
                style: Theme.of(context).textTheme.labelMedium?.copyWith(
                      color: onFill,
                      fontWeight: FontWeight.w600,
                    ),
              ),
            ],
          ),
        ),
      ),
    );
  }
}

/// Sea-state colour ramp keyed on significant wave height (metres), loosely
/// following the WMO sea-state scale: calm → slight → moderate → rough → high.
Color waveStateColor(double meters) {
  if (meters < 0.5) return const Color(0xFF1A9850); // calm — green
  if (meters < 1.25) return const Color(0xFF91CF60); // smooth/slight
  if (meters < 2.5) return const Color(0xFFFEE08B); // moderate — amber
  if (meters < 4.0) return const Color(0xFFFC8D59); // rough — orange
  return const Color(0xFFD73027); // high+ — red
}
