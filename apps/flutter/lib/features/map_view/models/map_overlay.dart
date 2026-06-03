import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

/// Where a map overlay is anchored.
enum MapOverlaySlot {
  /// Full-width bar flush to the bottom edge (e.g. a selection action bar).
  /// At most one occupant; highest priority wins.
  bottomBar,

  /// Collision-free floating column above the bottom bar, stacked by priority
  /// (highest sits nearest the edge). Self-hiding entries collapse out.
  bottomFloating,

  /// Floating column anchored to the top-centre (hints, transient banners).
  topCenter,
}

class MapOverlayContext {
  final WidgetRef ref;
  const MapOverlayContext({required this.ref});
}

typedef MapOverlayBuilder = Widget Function(MapOverlayContext ctx);

/// A persistent map overlay contributed by a feature — the same composition
/// seam as `MapLayerDescriptor` / `MapToolDescriptor`, but for the floating
/// chrome (status chips, recording panel, download toolbar, selection bar).
/// Features register one in `app/main.dart`; the host groups by [slot] and
/// lays them out collision-free, so no feature hand-places a `Positioned` in
/// `MainMapPage` anymore. See
/// `docs/architecture/2026-06-composition-overhaul-plan.md` (Phase 3, hardened).
class MapOverlayDescriptor {
  final String id;
  final MapOverlaySlot slot;

  /// Higher shows nearer its anchor edge (bottom slots) / first (top slot).
  final int priority;

  final MapOverlayBuilder build;

  const MapOverlayDescriptor({
    required this.id,
    required this.slot,
    required this.build,
    this.priority = 0,
  });
}
