import 'package:flutter/widgets.dart';
import 'package:flutter_map/flutter_map.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:latlong2/latlong.dart';

/// Everything a map tool needs to render and interact with the single shared
/// map: the build [ref] (for reading/watching providers) and the live
/// [mapController].
class MapToolContext {
  final WidgetRef ref;
  final MapController mapController;

  const MapToolContext({required this.ref, required this.mapController});
}

typedef MapToolLayersBuilder = List<Widget> Function(MapToolContext ctx);
typedef MapToolOverlayBuilder = Widget? Function(MapToolContext ctx);
typedef MapToolTapHandler = void Function(MapToolContext ctx, LatLng point);
typedef MapToolInteractionBuilder = InteractionOptions Function(
    MapToolContext ctx);
typedef MapToolPointerHandler<E extends PointerEvent> = void Function(
    MapToolContext ctx, E event, LatLng point);

/// A tool that mounts onto the *single* shared map rather than opening its own
/// full-screen map with a second `MapController`.
///
/// This is the composition seam for map tools — it mirrors
/// `ActivityKindDescriptor`: each tool feature exports one descriptor and
/// registers it in `app/main.dart`; the map host iterates the registry and
/// never names a concrete tool. Migrating the old pushed map pages (route
/// planning, measuring, …) to descriptors is what collapses the 5 map
/// instances into one. See
/// `docs/architecture/2026-06-composition-overhaul-plan.md` (Phase 4).
class MapToolDescriptor {
  /// Stable id used by [activeMapToolProvider].
  final String id;

  /// flutter_map layers merged into the live map while the tool is active.
  final MapToolLayersBuilder buildLayers;

  /// Optional full-area overlay (sheets, hint pills, a close button). The host
  /// wraps the returned widget in `Positioned.fill`.
  final MapToolOverlayBuilder? buildOverlay;

  /// Consumes map taps while active (e.g. dropping a waypoint).
  final MapToolTapHandler? onMapTap;

  /// Raw pointer handlers for freehand drawing / drag selection. Latitude/
  /// longitude is pre-resolved from the pointer's local position.
  final MapToolPointerHandler<PointerDownEvent>? onPointerDown;
  final MapToolPointerHandler<PointerMoveEvent>? onPointerMove;
  final MapToolPointerHandler<PointerUpEvent>? onPointerUp;

  /// Overrides map interaction while active (e.g. freezing pan during a drag).
  final MapToolInteractionBuilder? interaction;

  /// Called when the tool becomes / stops being active.
  final void Function(MapToolContext ctx)? onActivate;
  final void Function(MapToolContext ctx)? onDeactivate;

  const MapToolDescriptor({
    required this.id,
    required this.buildLayers,
    this.buildOverlay,
    this.onMapTap,
    this.onPointerDown,
    this.onPointerMove,
    this.onPointerUp,
    this.interaction,
    this.onActivate,
    this.onDeactivate,
  });
}
