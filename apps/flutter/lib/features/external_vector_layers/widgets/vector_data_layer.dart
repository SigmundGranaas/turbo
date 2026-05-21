import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_map/flutter_map.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import 'package:turbo/app/tokens.dart';
import '../data/vector_layer_notifier.dart';
import '../models/vector_feature.dart';
import '../models/vector_layer_source.dart';
import 'vector_feature_sheet.dart';

/// Renders a [VectorLayerSource] inside the flutter_map widget tree. The
/// layer listens to the map controller for pan/zoom-end events and refetches
/// features for the new viewport.
class VectorDataLayer extends ConsumerStatefulWidget {
  final VectorLayerSource source;
  final MapController mapController;
  final bool visible;

  /// Vector sources below this zoom return absurd feature counts (a WFS
  /// bbox spanning the whole country can be tens of thousands of trails).
  /// We skip fetching until the viewport is reasonably tight.
  ///
  /// Default 8 matches the WMS overlay's minZoom — by the time the user can
  /// see the raster preview, the vector layer can pull a manageable slice.
  final double minZoom;

  const VectorDataLayer({
    super.key,
    required this.source,
    required this.mapController,
    this.visible = true,
    this.minZoom = 8,
  });

  @override
  ConsumerState<VectorDataLayer> createState() => _VectorDataLayerState();
}

class _VectorDataLayerState extends ConsumerState<VectorDataLayer> {
  StreamSubscription<MapEvent>? _eventSub;
  final LayerHitNotifier<String> _hitNotifier = ValueNotifier(null);
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
  void dispose() {
    _hitNotifier.dispose();
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
    final notifier = ref.read(
        viewportVectorFeaturesProvider(widget.source.id).notifier);
    notifier.setSource(widget.source);
    notifier.requestBounds(camera.visibleBounds);
    _bootstrapped = true;
  }

  @override
  Widget build(BuildContext context) {
    if (!widget.visible) return const SizedBox.shrink();

    if (!_bootstrapped) {
      // Defer the first read until our listener has fired; we render an
      // empty layer in the meantime so the map tree doesn't flicker.
      return const SizedBox.shrink();
    }

    final async =
        ref.watch(viewportVectorFeaturesProvider(widget.source.id));
    final features = async.asData?.value ?? const <VectorFeature>[];
    if (features.isEmpty) return const SizedBox.shrink();

    final color = widget.source.color ??
        Theme.of(context)
            .colorScheme
            .tertiary
            .withValues(alpha: AppVectorOverlay.strokeAlpha);

    final polylines = <Polyline<String>>[];
    final polygons = <Polygon<String>>[];
    final featuresById = <String, VectorFeature>{};
    for (final f in features) {
      featuresById[f.id] = f;
      if (f.kind == VectorGeometryKind.line) {
        for (final ring in f.rings) {
          polylines.add(Polyline(
            points: ring,
            color: color,
            strokeWidth: AppVectorOverlay.lineStrokeWidth,
            hitValue: f.id,
          ));
        }
      } else {
        for (final ring in f.rings) {
          polygons.add(Polygon(
            points: ring,
            color: color.withValues(alpha: AppVectorOverlay.polygonFillAlpha),
            borderColor: color,
            borderStrokeWidth: AppVectorOverlay.polygonBorderStrokeWidth,
            hitValue: f.id,
          ));
        }
      }
    }

    return GestureDetector(
      onTap: () {
        final hit = _hitNotifier.value;
        if (hit == null || hit.hitValues.isEmpty) return;
        final feature = featuresById[hit.hitValues.first];
        if (feature != null) {
          showVectorFeatureSheet(
            context,
            source: widget.source,
            feature: feature,
          );
        }
      },
      child: Stack(
        children: [
          if (polygons.isNotEmpty)
            PolygonLayer<String>(
              hitNotifier: _hitNotifier,
              polygons: polygons,
            ),
          if (polylines.isNotEmpty)
            PolylineLayer<String>(
              hitNotifier: _hitNotifier,
              polylines: polylines,
            ),
        ],
      ),
    );
  }
}
