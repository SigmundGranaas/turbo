import 'package:flutter/material.dart';

/// A route waypoint dot.
///
/// Interaction model: a **tap removes** the stop; a **press-and-hold then drag
/// moves** it. Using the long-press gesture for the drag means it wins the
/// gesture arena outright once recognised, so the map never pans mid-move (no
/// raw pointer-down freeze needed). Start is green, end is flagged red,
/// intermediate stops are numbered in the theme primary.
class RouteWaypointDot extends StatelessWidget {
  final int index;
  final bool isStart;
  final bool isEnd;
  final bool isDragging;

  /// Hold recognised — caller should freeze the map / enlarge the dot.
  final VoidCallback onDragStart;

  /// Drag move — the global pointer position to project onto the map.
  final ValueChanged<Offset> onDragUpdate;

  /// Hold released / cancelled — caller should unfreeze the map.
  final VoidCallback onDragEnd;

  /// Tap — remove this stop.
  final VoidCallback onRemove;

  const RouteWaypointDot({
    super.key,
    required this.index,
    required this.isStart,
    required this.isEnd,
    required this.isDragging,
    required this.onDragStart,
    required this.onDragUpdate,
    required this.onDragEnd,
    required this.onRemove,
  });

  @override
  Widget build(BuildContext context) {
    final scheme = Theme.of(context).colorScheme;
    // Fixed, basemap-independent colours: green start, red end, neutral surface
    // for the via-points (NOT the theme primary/error, which become a pale
    // "skin" salmon in dark mode and vanish on the always-light topo).
    final fill = isStart
        ? const Color(0xFF2E7D32)
        : isEnd
            ? const Color(0xFFD32F2F)
            : scheme.surface;

    return GestureDetector(
        behavior: HitTestBehavior.opaque,
        // Tap removes the stop (also absorbs the tap so the map doesn't append).
        onTap: onRemove,
        // Press-and-hold then drag to reposition.
        onLongPressStart: (_) => onDragStart(),
        onLongPressMoveUpdate: (d) => onDragUpdate(d.globalPosition),
        onLongPressEnd: (_) => onDragEnd(),
        onLongPressCancel: onDragEnd,
        child: MouseRegion(
          cursor: SystemMouseCursors.grab,
          child: Center(
            child: AnimatedContainer(
              duration: const Duration(milliseconds: 130),
              curve: Curves.easeOut,
              width: isDragging ? 32 : 24,
              height: isDragging ? 32 : 24,
              decoration: BoxDecoration(
                color: fill,
                shape: BoxShape.circle,
                border: Border.all(color: Colors.white, width: 2.5),
                boxShadow: [
                  BoxShadow(
                    color: Colors.black.withValues(alpha: isDragging ? 0.45 : 0.25),
                    blurRadius: isDragging ? 8 : 3,
                    offset: const Offset(0, 1),
                  ),
                ],
              ),
              child: Center(child: _label()),
            ),
          ),
        ),
    );
  }

  Widget? _label() {
    if (isStart) {
      return const Icon(Icons.trip_origin, size: 13, color: Colors.white);
    }
    if (isEnd) {
      return const Icon(Icons.flag, size: 15, color: Colors.white);
    }
    return Text(
      '$index',
      style: const TextStyle(
        color: Colors.white,
        fontSize: 12,
        fontWeight: FontWeight.w700,
      ),
    );
  }
}
