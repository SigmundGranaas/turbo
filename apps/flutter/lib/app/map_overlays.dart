import 'package:flutter/widgets.dart';

import 'package:turbo/features/journey/api.dart' show ActiveOutingPanel;
import 'package:turbo/features/map_view/api.dart';
import 'package:turbo/features/markers/api.dart' show MarkerSelectionBar;
import 'package:turbo/features/tile_storage/offline_regions/api.dart';

/// The default floating-overlay stack, composed at the app shell. Mirrors the
/// layer/tool registries: each feature contributes a descriptor, the host lays
/// them out collision-free by slot. Adding/removing a map overlay happens here,
/// not in `MainMapPage`.
MapOverlayRegistry buildDefaultMapOverlayRegistry() {
  return MapOverlayRegistry([
    // Zero-footprint host: presents the unified detail sheet whenever something
    // is selected on the map (a long-pressed coordinate, a search result, …).
    MapOverlayDescriptor(
      id: 'entity_detail_host',
      slot: MapOverlaySlot.bottomFloating,
      priority: 0,
      build: (_) => const MapEntityDetailHost(),
    ),
    // Full-width action bar flush to the bottom edge.
    MapOverlayDescriptor(
      id: 'marker_selection_bar',
      slot: MapOverlaySlot.bottomBar,
      build: (_) => const MarkerSelectionBar(),
    ),
    // Floating column, highest priority nearest the edge:
    // outing panel (bottom) ← download (top). The outing panel is the SINGLE
    // widget for following + recording (replaces the old recording card +
    // separate "Following" chip stacking).
    MapOverlayDescriptor(
      id: 'download_toolbar',
      slot: MapOverlaySlot.bottomFloating,
      priority: 30,
      build: _downloadToolbar,
    ),
    MapOverlayDescriptor(
      id: 'active_outing',
      slot: MapOverlaySlot.bottomFloating,
      priority: 20,
      build: (_) => const Center(child: ActiveOutingPanel()),
    ),
    // Orientation-only chip (follow snap / compass) — hidden whenever an
    // outing is active, so it never stacks with the outing panel.
    MapOverlayDescriptor(
      id: 'orientation_chip',
      slot: MapOverlaySlot.bottomFloating,
      priority: 10,
      build: (_) => const ModeIndicator(),
    ),
  ]);
}

Widget _downloadToolbar(MapOverlayContext ctx) {
  final regions = ctx.ref.watch(offlineRegionsProvider).value;
  final hidden = ctx.ref.watch(hiddenDownloadsProvider);
  final active = regions
          ?.where((r) =>
              r.status == DownloadStatus.downloading && !hidden.contains(r.id))
          .toList() ??
      const [];
  if (active.isEmpty) return const SizedBox.shrink();
  final first = active.first;
  return Center(
    child: DownloadProgressToolbar(
      region: first,
      onHide: () => ctx.ref.read(hiddenDownloadsProvider.notifier).hide(first.id),
    ),
  );
}
