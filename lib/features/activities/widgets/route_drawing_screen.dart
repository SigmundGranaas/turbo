import 'package:flutter/material.dart';
import 'package:flutter_map/flutter_map.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:latlong2/latlong.dart';

import 'package:turbo/features/map_view/api.dart' show MapBase;
import 'package:turbo/features/tile_providers/api.dart' show activeTileLayersProvider;

/// Full-screen route drawing surface. Tap to add a vertex, drag a
/// vertex to move it, long-press to remove the nearest one. App-bar
/// actions: undo last, clear all. On save the widget pops the route as
/// a `List<LatLng>`; on cancel it pops `null`. The drawn polyline
/// renders live as the user works.
///
/// Lives in the activities shell so any kind's create screen can
/// launch it. No kind-specific knowledge here — the caller decides
/// what to do with the drawn route.
class RouteDrawingScreen extends ConsumerStatefulWidget {
  /// Initial center for the map; usually the long-press point that
  /// kicked off the create flow.
  final LatLng seedCenter;

  /// Optional pre-existing route to edit (vs draw from scratch).
  final List<LatLng>? initialRoute;

  /// Color hint for the polyline being drawn. Defaults to the
  /// theme's primary color.
  final Color? color;

  const RouteDrawingScreen({
    super.key,
    required this.seedCenter,
    this.initialRoute,
    this.color,
  });

  @override
  ConsumerState<RouteDrawingScreen> createState() => _RouteDrawingScreenState();
}

class _RouteDrawingScreenState extends ConsumerState<RouteDrawingScreen> {
  final MapController _map = MapController();
  // Attached to the wrapper around MapBase so we can resolve the map's
  // RenderBox directly. Using `context.findRenderObject()` from this
  // state finds the Scaffold, which is offset by the app bar and gives
  // wrong drag coordinates.
  final GlobalKey _mapKey = GlobalKey();
  late List<LatLng> _vertices;

  /// Set while the user is dragging a vertex. Lets us disable the
  /// map's own pan/zoom interaction so the gesture doesn't double-up.
  int? _draggingIdx;

  @override
  void initState() {
    super.initState();
    _vertices = List<LatLng>.from(widget.initialRoute ?? const []);
  }

  void _addVertex(LatLng p) => setState(() => _vertices.add(p));

  void _undo() {
    if (_vertices.isEmpty) return;
    setState(() => _vertices.removeLast());
  }

  void _clear() => setState(() => _vertices.clear());

  void _save() {
    if (_vertices.length < 2) return;
    Navigator.of(context).pop<List<LatLng>>(List<LatLng>.unmodifiable(_vertices));
  }

  /// Pan-update handler for a vertex drag. Converts the pointer's
  /// global position to a LatLng via the camera, mirrors the pattern
  /// `region_creation_page.dart` uses for resize handles.
  void _onVertexDrag(int idx, DragUpdateDetails details) {
    final renderBox = _mapKey.currentContext?.findRenderObject();
    if (renderBox is! RenderBox) return;
    final local = renderBox.globalToLocal(details.globalPosition);
    final newPoint = _map.camera.screenOffsetToLatLng(local);
    setState(() => _vertices[idx] = newPoint);
  }

  double _totalDistanceMeters() {
    if (_vertices.length < 2) return 0;
    final dist = const Distance();
    var sum = 0.0;
    for (var i = 1; i < _vertices.length; i++) {
      sum += dist.as(LengthUnit.Meter, _vertices[i - 1], _vertices[i]);
    }
    return sum;
  }

  String _formatDistance(double m) {
    if (m < 1000) return '${m.round()} m';
    return '${(m / 1000).toStringAsFixed(2)} km';
  }

  @override
  Widget build(BuildContext context) {
    final tileLayers = ref.watch(activeTileLayersProvider);
    final color = widget.color ?? Theme.of(context).colorScheme.primary;

    final layers = <Widget>[
      ...tileLayers,
      if (_vertices.length >= 2)
        PolylineLayer(polylines: [
          Polyline(
            points: _vertices,
            color: color.withValues(alpha: 0.85),
            strokeWidth: 4.5,
            strokeCap: StrokeCap.round,
            strokeJoin: StrokeJoin.round,
          ),
        ]),
      MarkerLayer(markers: [
        for (var i = 0; i < _vertices.length; i++)
          Marker(
            point: _vertices[i],
            // Slightly larger hit target than the visual dot so dragging
            // is forgiving on touch screens.
            width: 36,
            height: 36,
            child: _VertexDot(
              index: i,
              isStart: i == 0,
              isEnd: i == _vertices.length - 1 && _vertices.length > 1,
              isDragging: _draggingIdx == i,
              color: color,
              onPanStart: () => setState(() => _draggingIdx = i),
              onPanUpdate: (d) => _onVertexDrag(i, d),
              onPanEnd: () => setState(() => _draggingIdx = null),
              onLongPress: () => setState(() => _vertices.removeAt(i)),
            ),
          ),
      ]),
    ];

    final distance = _totalDistanceMeters();

    return Scaffold(
      appBar: AppBar(
        title: const Text('Draw route'),
        actions: [
          IconButton(
            tooltip: 'Undo last vertex',
            icon: const Icon(Icons.undo),
            onPressed: _vertices.isEmpty ? null : _undo,
          ),
          IconButton(
            tooltip: 'Clear all vertices',
            icon: const Icon(Icons.delete_outline),
            onPressed: _vertices.isEmpty ? null : _clear,
          ),
        ],
      ),
      body: Stack(children: [
        KeyedSubtree(
          key: _mapKey,
          child: MapBase(
            mapController: _map,
            mapLayers: layers,
            overlayWidgets: const [],
            initialCenter: widget.seedCenter,
            initialZoom: 13,
            onTap: (_, p) => _addVertex(p),
            // While a vertex is being dragged, freeze the map so the
            // gesture only moves the vertex (not the camera too).
            interactionOptions: InteractionOptions(
              flags: _draggingIdx != null
                  ? InteractiveFlag.none
                  : InteractiveFlag.all & ~InteractiveFlag.rotate,
            ),
          ),
        ),
        Positioned(
          top: 12,
          left: 12,
          child: Container(
            padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 8),
            decoration: BoxDecoration(
              color: Theme.of(context).colorScheme.surface.withValues(alpha: 0.92),
              borderRadius: BorderRadius.circular(16),
              boxShadow: [
                BoxShadow(
                  color: Colors.black.withValues(alpha: 0.1),
                  blurRadius: 4,
                  offset: const Offset(0, 1)),
              ],
            ),
            child: Row(mainAxisSize: MainAxisSize.min, children: [
              Icon(Icons.timeline_outlined, size: 18, color: color),
              const SizedBox(width: 6),
              Text('${_vertices.length} pts · ${_formatDistance(distance)}',
                style: Theme.of(context).textTheme.labelLarge),
            ]),
          ),
        ),
        Positioned(
          left: 12,
          right: 12,
          bottom: 16,
          child: Card(
            elevation: 4,
            child: Padding(
              padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 8),
              child: Row(children: [
                Expanded(child: Text(
                  _vertices.isEmpty
                    ? 'Tap the map to add vertices.'
                    : _vertices.length == 1
                      ? 'Add at least one more vertex.'
                      : 'Tap to extend, drag a dot to move it, long-press to remove.',
                  style: Theme.of(context).textTheme.bodySmall,
                )),
                const SizedBox(width: 8),
                TextButton(
                  onPressed: () => Navigator.of(context).pop<List<LatLng>>(null),
                  child: const Text('Cancel'),
                ),
                FilledButton.icon(
                  onPressed: _vertices.length >= 2 ? _save : null,
                  icon: const Icon(Icons.check, size: 18),
                  label: const Text('Use route'),
                ),
              ]),
            ),
          ),
        ),
      ]),
    );
  }
}

class _VertexDot extends StatelessWidget {
  final int index;
  final bool isStart;
  final bool isEnd;
  final bool isDragging;
  final Color color;
  final VoidCallback onPanStart;
  final ValueChanged<DragUpdateDetails> onPanUpdate;
  final VoidCallback onPanEnd;
  final VoidCallback onLongPress;

  const _VertexDot({
    required this.index,
    required this.isStart,
    required this.isEnd,
    required this.isDragging,
    required this.color,
    required this.onPanStart,
    required this.onPanUpdate,
    required this.onPanEnd,
    required this.onLongPress,
  });

  @override
  Widget build(BuildContext context) {
    final fill = isStart
        ? Colors.green
        : isEnd
            ? Colors.red.shade400
            : color;
    // The hit target (the GestureDetector child) is sized to the full
    // Marker (36×36); the visible dot inside is 22×22. The GestureDetector
    // absorbs taps too via the onTap handler so tapping the dot doesn't
    // bubble up to the map's onTap and add a duplicate vertex.
    return GestureDetector(
      behavior: HitTestBehavior.opaque,
      onTap: () {},
      onLongPress: onLongPress,
      onPanStart: (_) => onPanStart(),
      onPanUpdate: onPanUpdate,
      onPanEnd: (_) => onPanEnd(),
      child: MouseRegion(
        cursor: SystemMouseCursors.grab,
        child: Center(
          child: AnimatedContainer(
            duration: const Duration(milliseconds: 120),
            width: isDragging ? 28 : 22,
            height: isDragging ? 28 : 22,
            decoration: BoxDecoration(
              color: fill,
              shape: BoxShape.circle,
              border: Border.all(color: Colors.white, width: 2),
              boxShadow: [
                BoxShadow(
                  color: Colors.black.withValues(alpha: isDragging ? 0.4 : 0.25),
                  blurRadius: isDragging ? 6 : 2,
                  offset: const Offset(0, 1)),
              ],
            ),
            child: Center(
              child: Text(
                '${index + 1}',
                style: TextStyle(
                  color: Colors.white,
                  fontSize: isDragging ? 12 : 10,
                  fontWeight: FontWeight.w600,
                ),
              ),
            ),
          ),
        ),
      ),
    );
  }
}

/// Helper for callers needing the haversine distance of the drawn
/// route. Public so kind create screens can prefill stats without
/// re-importing latlong2's [Distance].
double routeDistanceMeters(List<LatLng> points) {
  if (points.length < 2) return 0;
  const dist = Distance();
  var sum = 0.0;
  for (var i = 1; i < points.length; i++) {
    sum += dist.as(LengthUnit.Meter, points[i - 1], points[i]);
  }
  return sum;
}
