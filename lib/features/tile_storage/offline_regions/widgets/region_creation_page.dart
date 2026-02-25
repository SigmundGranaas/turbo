import 'dart:async';
import 'package:flutter/material.dart';
import 'package:flutter_map/flutter_map.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:latlong2/latlong.dart';
import 'package:turbo/features/map_view/widgets/map_base.dart';
import 'package:turbo/features/tile_providers/api.dart';
import 'package:turbo/features/tile_storage/offline_regions/widgets/download_details_sheet.dart';
import 'package:turbo/core/widgets/map/controls/go_back_button.dart';

// OsmConfig is exported from tile_providers api.dart (already imported above)

enum SelectionMode { viewport, rectangle, draw }

class RegionCreationPage extends ConsumerStatefulWidget {
  final LatLng initialCenter;
  final double initialZoom;
  final TileLayer? activeTileLayer;

  const RegionCreationPage({
    super.key,
    required this.initialCenter,
    required this.initialZoom,
    this.activeTileLayer,
  });

  @override
  ConsumerState<RegionCreationPage> createState() => _RegionCreationPageState();
}

class _RegionCreationPageState extends ConsumerState<RegionCreationPage>
    with TickerProviderStateMixin {
  late MapController _mapController;
  StreamSubscription<MapEvent>? _mapEventSubscription;

  SelectionMode _selectionMode = SelectionMode.viewport;
  LatLngBounds? _selectionBounds;
  List<LatLng> _drawnPoints = [];
  int? _draggedHandleIndex;
  bool _isDrawing = false;
  bool _isSelectionValid = false;

  @override
  void initState() {
    super.initState();
    _mapController = MapController();
    _mapEventSubscription = _mapController.mapEventStream.listen(_onMapEvent);
  }

  @override
  void dispose() {
    _mapEventSubscription?.cancel();
    _mapController.dispose();
    super.dispose();
  }

  void _onMapEvent(MapEvent event) {
    if (event is MapEventMoveEnd || event is MapEventFlingAnimationEnd) {
      if (mounted) _updateSelectionFromMode();
    }
  }

  void _updateSelectionFromMode() {
    if (_selectionMode == SelectionMode.viewport) {
      _updateViewportSelection();
    }
  }

  void _setSelectionBounds(LatLngBounds? bounds) {
    setState(() {
      _selectionBounds = bounds;
      _isSelectionValid = bounds != null &&
          (_drawnPoints.length > 2 || _selectionMode != SelectionMode.draw);
    });
  }

  void _updateViewportSelection() {
    if (!mounted) return;
    _setSelectionBounds(_mapController.camera.visibleBounds);
  }

  void _initializeRectangle() {
    if (!mounted) return;
    final bounds = _mapController.camera.visibleBounds;
    final latInset = (bounds.north - bounds.south) * 0.15;
    final lngInset = (bounds.east - bounds.west) * 0.15;
    _setSelectionBounds(LatLngBounds(
      LatLng(bounds.south + latInset, bounds.west + lngInset),
      LatLng(bounds.north - latInset, bounds.east - lngInset),
    ));
  }

  void _onModeChanged(SelectionMode mode) {
    setState(() {
      _selectionMode = mode;
      _drawnPoints = [];
      _isDrawing = false;

      if (mode == SelectionMode.rectangle) {
        _initializeRectangle();
      } else if (mode == SelectionMode.viewport) {
        _updateViewportSelection();
      } else {
        _setSelectionBounds(null); // Clear selection for draw mode until finished
      }
    });
  }

  void _onPointerDown(PointerDownEvent event, LatLng latlng) {
    setState(() {
      _isDrawing = true;
      _drawnPoints = [latlng];
      _setSelectionBounds(null); // Invalidate bounds while drawing
    });
  }

  void _onPointerMove(PointerMoveEvent event, LatLng latlng) {
    if (!_isDrawing) return;
    setState(() {
      _drawnPoints.add(latlng);
    });
  }

  void _onPointerUp(PointerUpEvent event, LatLng latlng) {
    if (!_isDrawing) return;
    setState(() {
      _isDrawing = false;
      if (_drawnPoints.length > 2) {
        _setSelectionBounds(LatLngBounds.fromPoints(_drawnPoints));
      } else {
        // Not a valid polygon, clear it.
        _drawnPoints.clear();
        _setSelectionBounds(null);
      }
    });
  }

  List<Marker> _getRectangleHandles() {
    if (_selectionBounds == null) return [];
    final sw = _selectionBounds!.southWest, nw = _selectionBounds!.northWest;
    final ne = _selectionBounds!.northEast, se = _selectionBounds!.southEast;
    final handles = [sw, nw, ne, se];

    return handles.asMap().entries.map((entry) {
      int idx = entry.key;
      return Marker(
        point: entry.value,
        width: 24,
        height: 24,
        child: DraggableHandle(
          onPanStart: () => _draggedHandleIndex = idx,
          onPanUpdate: (details) {
            final mapRenderBox = context.findRenderObject() as RenderBox;
            final localPt = mapRenderBox.globalToLocal(details.globalPosition);
            final newPoint = _mapController.camera.screenOffsetToLatLng(localPt);
            _updateRectangle(newPoint);
          },
          onPanEnd: () => _draggedHandleIndex = null,
        ),
      );
    }).toList();
  }

  void _updateRectangle(LatLng newPoint) {
    if (_draggedHandleIndex == null || _selectionBounds == null) return;
    var west = _selectionBounds!.west, east = _selectionBounds!.east;
    var north = _selectionBounds!.north, south = _selectionBounds!.south;

    switch (_draggedHandleIndex) {
      case 0:
        south = newPoint.latitude;
        west = newPoint.longitude;
        break;
      case 1:
        north = newPoint.latitude;
        west = newPoint.longitude;
        break;
      case 2:
        north = newPoint.latitude;
        east = newPoint.longitude;
        break;
      case 3:
        south = newPoint.latitude;
        east = newPoint.longitude;
        break;
    }
    _setSelectionBounds(LatLngBounds(LatLng(south, west), LatLng(north, east)));
  }

  @override
  Widget build(BuildContext context) {
    final backgroundLayer = widget.activeTileLayer ??
        ref.watch(activeTileLayersProvider).firstOrNull ??
        TileLayer(urlTemplate: OsmConfig().urlTemplate); // Fallback

    return Scaffold(
      body: MapBase(
        mapController: _mapController,
        initialCenter: widget.initialCenter,
        initialZoom: widget.initialZoom,
        onMapReady: _updateViewportSelection,
        onPointerDown: _selectionMode == SelectionMode.draw ? _onPointerDown : null,
        onPointerMove: _selectionMode == SelectionMode.draw ? _onPointerMove : null,
        onPointerUp: _selectionMode == SelectionMode.draw ? _onPointerUp : null,
        interactionOptions: InteractionOptions(
          flags: _isDrawing
              ? InteractiveFlag.none // Disable map movement while drawing
              : InteractiveFlag.all & ~InteractiveFlag.rotate,
        ),
        mapLayers: [
          backgroundLayer,
          if (_selectionBounds != null && _selectionMode != SelectionMode.draw)
            PolygonLayer(polygons: [
              Polygon(
                points: [
                  _selectionBounds!.southWest,
                  _selectionBounds!.northWest,
                  _selectionBounds!.northEast,
                  _selectionBounds!.southEast,
                ],
                color: Colors.blue.withValues(alpha: 0.1),
                borderColor: Colors.blue,
                borderStrokeWidth: 2,
              ),
            ]),
          if (_drawnPoints.isNotEmpty)
            PolygonLayer(polygons: [
              Polygon(
                points: _drawnPoints,
                color: Colors.blue.withValues(alpha: 0.1),
                borderColor: Colors.blue,
                borderStrokeWidth: 2,
              )
            ]),
          if (_selectionMode == SelectionMode.rectangle)
            MarkerLayer(markers: _getRectangleHandles()),
        ],
        overlayWidgets: [
          const Positioned(
            top: 16,
            left: 16,
            child: GoBackButton(),
          ),
          Positioned(
            bottom: 32,
            left: 16,
            right: 16,
            child: Center(
              child: CreationControls(
                selectionMode: _selectionMode,
                isSelectionValid: _isSelectionValid,
                onModeChanged: _onModeChanged,
                onClearDrawing: () => setState(() {
                  _drawnPoints.clear();
                  _setSelectionBounds(null);
                }),
                onNext: () {
                  if (_selectionBounds != null) {
                    showModalBottomSheet(
                      context: context,
                      isScrollControlled: true,
                      useSafeArea: true,
                      builder: (_) => DownloadDetailsSheet(bounds: _selectionBounds!),
                    );
                  }
                },
              ),
            ),
          ),
        ],
      ),
    );
  }
}

class DraggableHandle extends StatelessWidget {
  final VoidCallback onPanStart;
  final Function(DragUpdateDetails) onPanUpdate;
  final VoidCallback onPanEnd;

  const DraggableHandle(
      {super.key,
        required this.onPanStart,
        required this.onPanUpdate,
        required this.onPanEnd});

  @override
  Widget build(BuildContext context) {
    return GestureDetector(
      onPanStart: (_) => onPanStart(),
      onPanUpdate: onPanUpdate,
      onPanEnd: (_) => onPanEnd(),
      child: MouseRegion(
        cursor: SystemMouseCursors.move,
        child: Container(
          width: 24,
          height: 24,
          decoration: BoxDecoration(
            color: Theme.of(context).colorScheme.primary,
            shape: BoxShape.circle,
            border: Border.all(color: Colors.white, width: 2),
            boxShadow: [BoxShadow(color: Colors.black.withValues(alpha: 0.3), blurRadius: 5)],
          ),
        ),
      ),
    );
  }
}

class CreationControls extends StatelessWidget {
  final SelectionMode selectionMode;
  final ValueChanged<SelectionMode> onModeChanged;
  final VoidCallback onClearDrawing;
  final VoidCallback onNext;
  final bool isSelectionValid;

  const CreationControls({
    super.key,
    required this.selectionMode,
    required this.onModeChanged,
    required this.onClearDrawing,
    required this.onNext,
    required this.isSelectionValid,
  });

  @override
  Widget build(BuildContext context) {
    final colorScheme = Theme.of(context).colorScheme;

    Widget button(
        {required String tooltip, required IconData icon, required SelectionMode mode}) {
      final isSelected = selectionMode == mode;
      return IconButton(
        tooltip: tooltip,
        iconSize: 20,
        style: isSelected
            ? IconButton.styleFrom(
          backgroundColor: colorScheme.primary,
          foregroundColor: colorScheme.onPrimary,
          padding: const EdgeInsets.all(10),
        )
            : IconButton.styleFrom(
          backgroundColor: colorScheme.surfaceContainer,
          foregroundColor: colorScheme.onSurfaceVariant,
          padding: const EdgeInsets.all(10),
        ),
        icon: Icon(icon),
        onPressed: () => onModeChanged(mode),
      );
    }

    return Card(
      elevation: 4,
      shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(100)),
      color: colorScheme.surfaceContainer,
      child: Padding(
        padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 8),
        child: Row(
          mainAxisSize: MainAxisSize.min,
          children: [
            button(
                tooltip: "Select by Viewport",
                icon: Icons.fullscreen,
                mode: SelectionMode.viewport),
            const SizedBox(width: 6),
            button(
                tooltip: "Select by Rectangle",
                icon: Icons.crop_square,
                mode: SelectionMode.rectangle),
            const SizedBox(width: 6),
            button(
                tooltip: "Draw Area", icon: Icons.draw_outlined, mode: SelectionMode.draw),
            if (selectionMode == SelectionMode.draw) ...[
              const SizedBox(width: 6),
              IconButton(
                tooltip: "Clear Drawing",
                iconSize: 20,
                style: IconButton.styleFrom(
                  backgroundColor: colorScheme.surfaceContainer,
                  foregroundColor: colorScheme.onSurfaceVariant,
                  padding: const EdgeInsets.all(10),
                ),
                icon: const Icon(Icons.clear_all),
                onPressed: onClearDrawing,
              ),
            ],
            const SizedBox(width: 6),
            const VerticalDivider(width: 1, thickness: 1, indent: 8, endIndent: 8),
            const SizedBox(width: 6),
            FilledButton.tonal(
              style: FilledButton.styleFrom(
                padding: const EdgeInsets.symmetric(horizontal: 16),
                textStyle: Theme.of(context).textTheme.labelLarge,
              ),
              onPressed: isSelectionValid ? onNext : null,
              child: const Text("Next"),
            ),
          ],
        ),
      ),
    );
  }
}