import 'dart:async';
import 'dart:math' as math;

import 'package:flutter/material.dart';
import 'package:turbo/core/widgets/exclusive_sheet.dart';
import 'package:flutter_map/flutter_map.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import 'package:turbo/app/tokens.dart';
import 'package:turbo/features/external_vector_layers/api.dart';
import '../data/mvt_tile_repository.dart';
import '../models/mvt_layer_source.dart';

/// Renders an [MvtLayerSource] over flutter_map. Listens to the map
/// controller for pan/zoom-end events and refetches the visible slippy
/// tiles. Sibling to `VectorDataLayer` but operates per-tile (z/x/y)
/// rather than per-bbox, since MVT is tile-addressed at source.
class MvtDataLayer extends ConsumerStatefulWidget {
  final MvtLayerSource source;
  final MapController mapController;
  final bool visible;

  const MvtDataLayer({
    super.key,
    required this.source,
    required this.mapController,
    this.visible = true,
  });

  @override
  ConsumerState<MvtDataLayer> createState() => _MvtDataLayerState();
}

class _MvtDataLayerState extends ConsumerState<MvtDataLayer> {
  StreamSubscription<MapEvent>? _eventSub;
  final LayerHitNotifier<String> _hitNotifier = ValueNotifier(null);
  List<VectorFeature> _features = const [];
  bool _loading = false;
  int _refreshSeq = 0;

  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addPostFrameCallback((_) {
      _eventSub = widget.mapController.mapEventStream.listen((event) {
        if (event is MapEventMoveEnd ||
            event is MapEventFlingAnimationEnd ||
            event is MapEventRotateEnd ||
            event is MapEventDoubleTapZoomEnd) {
          _refresh();
        }
      });
      _refresh();
    });
  }

  @override
  void didUpdateWidget(covariant MvtDataLayer oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (oldWidget.source.id != widget.source.id || oldWidget.visible != widget.visible) {
      _refresh();
    }
  }

  @override
  void dispose() {
    _eventSub?.cancel();
    super.dispose();
  }

  Future<void> _refresh() async {
    if (!mounted || !widget.visible) return;
    final camera = widget.mapController.camera;
    final z = camera.zoom.floor();
    if (z < widget.source.minZoom || z > widget.source.maxZoom) {
      if (_features.isNotEmpty) setState(() => _features = const []);
      return;
    }
    final bounds = camera.visibleBounds;
    final tileSize = 1 << z;
    int lonToX(double lon) {
      final x = ((lon + 180.0) / 360.0 * tileSize).floor();
      return x.clamp(0, tileSize - 1);
    }

    int latToY(double lat) {
      final latRad = lat * math.pi / 180.0;
      final n = math.log(math.tan(latRad) + 1.0 / math.cos(latRad));
      final y = ((1.0 - n / math.pi) / 2.0 * tileSize).floor();
      return y.clamp(0, tileSize - 1);
    }

    final minX = lonToX(bounds.west);
    final maxX = lonToX(bounds.east);
    final minY = latToY(bounds.north);
    final maxY = latToY(bounds.south);
    // Cap range so a fast pan over a wide area doesn't fan out hundreds
    // of in-flight fetches; the visible-bbox refetch on movement-end is
    // what should land the right tile set.
    if ((maxX - minX) * (maxY - minY) > 64) return;

    final seq = ++_refreshSeq;
    if (!_loading) setState(() => _loading = true);
    final repo = ref.read(mvtTileRepositoryProvider);
    final features = await repo.tiles(
      widget.source,
      z: z,
      minX: minX,
      maxX: maxX,
      minY: minY,
      maxY: maxY,
    );
    if (!mounted || seq != _refreshSeq) return;
    setState(() {
      _features = features;
      _loading = false;
    });
  }

  @override
  Widget build(BuildContext context) {
    if (!widget.visible || _features.isEmpty) return const SizedBox.shrink();

    final color = widget.source.color;
    final polylines = <Polyline<String>>[];
    final polygons = <Polygon<String>>[];
    final featuresById = <String, VectorFeature>{};
    for (final f in _features) {
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
        if (feature == null) return;
        final builder = widget.source.sheetBuilder;
        if (builder == null) return;
        showExclusiveSheet(
          context,
          builder: (ctx) => builder(ctx, feature),
        );
      },
      child: Stack(children: [
        if (polygons.isNotEmpty)
          PolygonLayer<String>(hitNotifier: _hitNotifier, polygons: polygons),
        if (polylines.isNotEmpty)
          PolylineLayer<String>(hitNotifier: _hitNotifier, polylines: polylines),
      ]),
    );
  }
}
