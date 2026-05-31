import 'package:flutter/material.dart';

/// A draggable route waypoint dot. Adopts the app's route-drawing pattern
/// (see `activities/route_drawing_screen.dart`): a generous hit target,
/// tap absorbed so it doesn't add a stray stop, long-press to remove, and
/// pan to reposition. The visual grows + lifts while dragging.
///
/// Start is green, end is flagged red, intermediate stops are numbered in
/// the theme primary.
class RouteWaypointDot extends StatelessWidget {
  final int index;
  final bool isStart;
  final bool isEnd;
  final bool isDragging;
  final VoidCallback onPanStart;
  final ValueChanged<DragUpdateDetails> onPanUpdate;
  final VoidCallback onPanEnd;
  final VoidCallback onLongPress;

  const RouteWaypointDot({
    super.key,
    required this.index,
    required this.isStart,
    required this.isEnd,
    required this.isDragging,
    required this.onPanStart,
    required this.onPanUpdate,
    required this.onPanEnd,
    required this.onLongPress,
  });

  @override
  Widget build(BuildContext context) {
    final scheme = Theme.of(context).colorScheme;
    final fill = isStart
        ? const Color(0xFF2E7D32)
        : isEnd
            ? scheme.error
            : scheme.primary;

    // Hit target is the full Marker (38×38); the visible dot is smaller.
    // onTap is absorbed so tapping a dot doesn't bubble to the map and add
    // a duplicate stop.
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
            duration: const Duration(milliseconds: 130),
            curve: Curves.easeOut,
            width: isDragging ? 30 : 24,
            height: isDragging ? 30 : 24,
            decoration: BoxDecoration(
              color: fill,
              shape: BoxShape.circle,
              border: Border.all(color: scheme.surface, width: 2.5),
              boxShadow: [
                BoxShadow(
                  color: Colors.black.withValues(alpha: isDragging ? 0.4 : 0.22),
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

  Widget _label() {
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
