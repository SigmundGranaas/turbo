import 'package:flutter/widgets.dart';
import 'package:flutter_map/flutter_map.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

/// Context handed to a map layer builder: the build [ref] and the shared
/// [mapController] (layers like SavedPaths / Ocean / Viewport markers need it).
class MapLayerContext {
  final WidgetRef ref;
  final MapController mapController;

  const MapLayerContext({required this.ref, required this.mapController});
}

/// Returns the flutter_map layer widgets this descriptor contributes, in
/// render order. Most return a single layer; grouped overlays (e.g. the trail
/// vector sources) return several.
typedef MapLayerBuilder = List<Widget> Function(MapLayerContext ctx);

/// A stackable map layer contributed by a feature.
///
/// The composition seam for map layers — mirrors `ActivityKindDescriptor` and
/// `MapToolDescriptor`. Registered (in order) in `app/main.dart`; the map host
/// iterates the registry instead of hand-listing every feature's layer. Adding
/// a layer feature no longer edits `MainMapPage`. See
/// `docs/architecture/2026-06-composition-overhaul-plan.md` (Phase 5).
class MapLayerDescriptor {
  /// Stable id (debugging / future visibility toggles).
  final String id;

  /// Builds the layer widget(s). Registry order is render order (z-order).
  final MapLayerBuilder build;

  const MapLayerDescriptor({required this.id, required this.build});
}
