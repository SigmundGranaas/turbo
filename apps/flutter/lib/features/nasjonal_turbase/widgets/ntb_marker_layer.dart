import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_map/flutter_map.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import 'package:turbo/core/widgets/exclusive_sheet.dart';
import 'package:turbo/core/widgets/map/map_marker_pin.dart';
import '../models/ntb_poi.dart';
import '../providers/ntb_providers.dart';
import 'ntb_info_sheet.dart';

/// Viewport-reactive marker layer for Nasjonal Turbase (ut.no / DNT) cabins and
/// trips. Mirrors the `MvtDataLayer` refresh pattern: it refetches on
/// movement-end and renders a [MapMarkerPin] per POI. Tapping a pin opens the
/// info sheet and — for trips — triggers the animated route reveal.
class NtbMarkerLayer extends ConsumerStatefulWidget {
  final MapController mapController;
  final bool visible;

  const NtbMarkerLayer({
    super.key,
    required this.mapController,
    this.visible = true,
  });

  @override
  ConsumerState<NtbMarkerLayer> createState() => _NtbMarkerLayerState();
}

class _NtbMarkerLayerState extends ConsumerState<NtbMarkerLayer> {
  StreamSubscription<MapEvent>? _eventSub;

  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addPostFrameCallback((_) {
      _eventSub = widget.mapController.mapEventStream.listen((event) {
        if (event is MapEventMoveEnd ||
            event is MapEventFlingAnimationEnd ||
            event is MapEventRotateEnd ||
            event is MapEventDoubleTapZoomEnd ||
            event is MapEventScrollWheelZoom) {
          _refresh();
        }
      });
      _refresh();
    });
  }

  @override
  void didUpdateWidget(covariant NtbMarkerLayer oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (oldWidget.visible != widget.visible) _refresh();
  }

  @override
  void dispose() {
    _eventSub?.cancel();
    super.dispose();
  }

  void _refresh() {
    if (!mounted || !widget.visible) return;
    final camera = widget.mapController.camera;
    ref
        .read(ntbViewportPoisProvider.notifier)
        .load(camera.visibleBounds, camera.zoom);
  }

  @override
  Widget build(BuildContext context) {
    if (!widget.visible) return const SizedBox.shrink();
    final pois = ref.watch(ntbViewportPoisProvider);
    if (pois.isEmpty) return const SizedBox.shrink();

    return MarkerLayer(
      markers: [
        for (final poi in pois)
          Marker(
            width: MapMarkerPin.baseWidth,
            height: MapMarkerPin.baseHeight,
            point: poi.position,
            alignment: Alignment.bottomCenter,
            child: MapMarkerPin(
              icon: _iconFor(poi.type),
              accent: _accentFor(poi.type),
              title: poi.title,
              onTap: () => _onTap(poi),
            ),
          ),
      ],
    );
  }

  void _onTap(NtbPoi poi) {
    // Kick off the route reveal (no-op/clear for non-trips) before the sheet so
    // the line is already drawing behind it.
    ref.read(ntbSelectedRouteProvider.notifier).select(poi);
    showExclusiveSheet<void>(
      context,
      backgroundColor: Colors.transparent,
      builder: (_) => NtbInfoSheet(poi: poi),
    ).then((_) {
      // Clear the presented route once the user dismisses the sheet.
      if (mounted) ref.read(ntbSelectedRouteProvider.notifier).clear();
    });
  }

  static IconData _iconFor(NtbPoiType type) => switch (type) {
        NtbPoiType.cabin => Icons.cabin,
        NtbPoiType.trip => Icons.hiking,
        NtbPoiType.place => Icons.place,
      };

  static Color _accentFor(NtbPoiType type) => switch (type) {
        NtbPoiType.cabin => const Color(0xFF6D4C41),
        NtbPoiType.trip => const Color(0xFF2E7D32),
        NtbPoiType.place => const Color(0xFF1565C0),
      };
}
