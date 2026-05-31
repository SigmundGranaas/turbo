import 'package:flutter/material.dart';

/// A draggable route waypoint dot.
///
/// Dragging a marker on a [FlutterMap] fights the map's own pan in the
/// gesture arena — relying on `onPanStart` to then freeze the map loses
/// the first frames (the map pans instead). So we freeze on pointer
/// **down** via a [Listener] (`onDragStart`): by the time a pan could
/// begin, the map already has no drag recognizer, so the dot wins
/// uncontested. Tap is absorbed (so it doesn't add a stray stop) and
/// long-press removes the stop. Start is green, end is flagged red,
/// intermediate stops are numbered in the theme primary.
class RouteWaypointDot extends StatelessWidget {
  final int index;
  final bool isStart;
  final bool isEnd;
  final bool isDragging;

  /// Pointer went down on the dot — caller should freeze the map.
  final VoidCallback onDragStart;
  final ValueChanged<DragUpdateDetails> onDragUpdate;

  /// Pointer released / cancelled — caller should unfreeze the map.
  final VoidCallback onDragEnd;
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
    final fill = isStart
        ? const Color(0xFF2E7D32)
        : isEnd
            ? scheme.error
            : scheme.primary;

    return Listener(
      onPointerDown: (_) => onDragStart(),
      onPointerUp: (_) => onDragEnd(),
      onPointerCancel: (_) => onDragEnd(),
      child: GestureDetector(
        behavior: HitTestBehavior.opaque,
        onTap: () {}, // absorb so the map's onTap doesn't add a stop
        onLongPress: onRemove,
        onPanUpdate: onDragUpdate,
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
