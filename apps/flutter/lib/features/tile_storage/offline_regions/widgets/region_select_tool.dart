import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_map/flutter_map.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import 'package:turbo/app/l10n/app_localizations.dart';
import 'package:turbo/app/map_overlay_tokens.dart';
import 'package:turbo/app/tokens.dart';
import 'package:turbo/core/widgets/app_button.dart';
import 'package:turbo/core/widgets/exclusive_sheet.dart';
import 'package:turbo/core/widgets/map/buttons/map_control_button_base.dart';
import 'package:turbo/features/map_view/api.dart';

import '../data/region_selection_notifier.dart';
import 'download_details_sheet.dart';

const String regionSelectToolId = 'region_select';

/// Offline-region selection as an in-place [MapToolDescriptor] on the shared
/// map — replaces the old full-screen `RegionCreationPage` + its own
/// `MapController`. Selection state lives in [regionSelectionProvider].
final regionSelectTool = MapToolDescriptor(
  id: regionSelectToolId,
  buildLayers: (ctx) => [RegionToolLayers(mapController: ctx.mapController)],
  buildOverlay: (ctx) => RegionToolOverlay(mapController: ctx.mapController),
  onPointerDown: (ctx, e, p) {
    if (ctx.ref.read(regionSelectionProvider).mode == SelectionMode.draw) {
      ctx.ref.read(regionSelectionProvider.notifier).pointerDown(p);
    }
  },
  onPointerMove: (ctx, e, p) {
    if (ctx.ref.read(regionSelectionProvider).mode == SelectionMode.draw) {
      ctx.ref.read(regionSelectionProvider.notifier).pointerMove(p);
    }
  },
  onPointerUp: (ctx, e, p) {
    if (ctx.ref.read(regionSelectionProvider).mode == SelectionMode.draw) {
      ctx.ref.read(regionSelectionProvider.notifier).pointerUp();
    }
  },
  interaction: (ctx) {
    final drawing = ctx.ref.watch(regionSelectionProvider).isDrawing;
    return InteractionOptions(
      flags: drawing
          ? InteractiveFlag.none
          : InteractiveFlag.all & ~InteractiveFlag.rotate,
    );
  },
  onActivate: (ctx) => ctx.ref
      .read(regionSelectionProvider.notifier)
      .setMode(SelectionMode.viewport, ctx.mapController.camera.visibleBounds),
  onDeactivate: (ctx) =>
      ctx.ref.read(regionSelectionProvider.notifier).reset(),
);

class RegionToolLayers extends ConsumerWidget {
  final MapController mapController;
  const RegionToolLayers({super.key, required this.mapController});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final state = ref.watch(regionSelectionProvider);
    final notifier = ref.read(regionSelectionProvider.notifier);
    final bounds = state.bounds;

    List<Marker> handles() {
      if (bounds == null) return const [];
      final corners = [
        bounds.southWest,
        bounds.northWest,
        bounds.northEast,
        bounds.southEast,
      ];
      return corners.asMap().entries.map((e) {
        return Marker(
          point: e.value,
          width: 24,
          height: 24,
          child: _DraggableHandle(
            onPanStart: () => notifier.startHandleDrag(e.key),
            onPanUpdate: (d) {
              final p =
                  mapController.camera.screenOffsetToLatLng(d.globalPosition);
              notifier.updateHandle(p);
            },
            onPanEnd: notifier.endHandleDrag,
          ),
        );
      }).toList();
    }

    return Stack(
      children: [
        if (bounds != null && state.mode != SelectionMode.draw)
          PolygonLayer(polygons: [
            Polygon(
              points: [
                bounds.southWest,
                bounds.northWest,
                bounds.northEast,
                bounds.southEast,
              ],
              color: MapOverlayTokens.selectionFill,
              borderColor: MapOverlayTokens.selectionBorder,
              borderStrokeWidth: 2,
            ),
          ]),
        if (state.drawnPoints.isNotEmpty)
          PolygonLayer(polygons: [
            Polygon(
              points: state.drawnPoints,
              color: MapOverlayTokens.selectionFill,
              borderColor: MapOverlayTokens.selectionBorder,
              borderStrokeWidth: 2,
            ),
          ]),
        if (state.mode == SelectionMode.rectangle)
          MarkerLayer(markers: handles()),
      ],
    );
  }
}

class RegionToolOverlay extends ConsumerStatefulWidget {
  final MapController mapController;
  const RegionToolOverlay({super.key, required this.mapController});

  @override
  ConsumerState<RegionToolOverlay> createState() => _RegionToolOverlayState();
}

class _RegionToolOverlayState extends ConsumerState<RegionToolOverlay> {
  StreamSubscription<MapEvent>? _sub;

  @override
  void initState() {
    super.initState();
    // Track the camera so viewport-mode selection follows pan/zoom.
    _sub = widget.mapController.mapEventStream.listen((event) {
      if (event is MapEventMoveEnd || event is MapEventFlingAnimationEnd) {
        ref
            .read(regionSelectionProvider.notifier)
            .updateViewport(widget.mapController.camera.visibleBounds);
      }
    });
  }

  @override
  void dispose() {
    _sub?.cancel();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final state = ref.watch(regionSelectionProvider);
    final notifier = ref.read(regionSelectionProvider.notifier);
    final scheme = Theme.of(context).colorScheme;

    return Stack(
      children: [
        Positioned(
          top: 16,
          left: 16,
          child: MapControlButtonBase(
            onPressed: () =>
                ref.read(activeMapToolProvider.notifier).deactivate(),
            child: Icon(Icons.close, color: scheme.primary),
          ),
        ),
        Positioned(
          bottom: 32,
          left: 16,
          right: 16,
          child: SafeArea(
            top: false,
            child: Center(
              child: RegionCreationControls(
                selectionMode: state.mode,
                isSelectionValid: state.isValid,
                onModeChanged: (mode) => notifier.setMode(
                    mode, widget.mapController.camera.visibleBounds),
                onClearDrawing: notifier.clearDrawing,
                onNext: () {
                  final bounds = state.bounds;
                  if (bounds != null) {
                    showExclusiveSheet<void>(
                      context,
                      builder: (_) => DownloadDetailsSheet(bounds: bounds),
                    );
                  }
                },
              ),
            ),
          ),
        ),
      ],
    );
  }
}

class _DraggableHandle extends StatelessWidget {
  final VoidCallback onPanStart;
  final Function(DragUpdateDetails) onPanUpdate;
  final VoidCallback onPanEnd;

  const _DraggableHandle({
    required this.onPanStart,
    required this.onPanUpdate,
    required this.onPanEnd,
  });

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
            border: Border.all(color: MapOverlayTokens.handleBorder, width: 2),
            boxShadow: const [
              BoxShadow(color: MapOverlayTokens.handleShadow, blurRadius: 5),
            ],
          ),
        ),
      ),
    );
  }
}

class RegionCreationControls extends StatelessWidget {
  final SelectionMode selectionMode;
  final ValueChanged<SelectionMode> onModeChanged;
  final VoidCallback onClearDrawing;
  final VoidCallback onNext;
  final bool isSelectionValid;

  const RegionCreationControls({
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
    final l10n = context.l10n;

    Widget toggleButton({
      required String tooltip,
      required IconData icon,
      required SelectionMode mode,
    }) {
      final isSelected = selectionMode == mode;
      return IconButton(
        tooltip: tooltip,
        iconSize: 20,
        style: IconButton.styleFrom(
          backgroundColor:
              isSelected ? colorScheme.primary : colorScheme.surfaceContainer,
          foregroundColor:
              isSelected ? colorScheme.onPrimary : colorScheme.onSurfaceVariant,
        ),
        icon: Icon(icon),
        onPressed: () => onModeChanged(mode),
      );
    }

    return Card(
      elevation: AppElevation.floating,
      shape: const StadiumBorder(),
      color: colorScheme.surfaceContainer,
      child: Padding(
        padding: const EdgeInsets.symmetric(
            horizontal: AppSpacing.m, vertical: AppSpacing.s),
        child: Row(
          mainAxisSize: MainAxisSize.min,
          children: [
            toggleButton(
                tooltip: l10n.selectByViewport,
                icon: Icons.fullscreen,
                mode: SelectionMode.viewport),
            const SizedBox(width: AppSpacing.xs + 2),
            toggleButton(
                tooltip: l10n.selectByRectangle,
                icon: Icons.crop_square,
                mode: SelectionMode.rectangle),
            const SizedBox(width: AppSpacing.xs + 2),
            toggleButton(
                tooltip: l10n.drawArea,
                icon: Icons.draw_outlined,
                mode: SelectionMode.draw),
            if (selectionMode == SelectionMode.draw) ...[
              const SizedBox(width: AppSpacing.xs + 2),
              IconButton(
                tooltip: l10n.clearDrawing,
                iconSize: 20,
                style: IconButton.styleFrom(
                  backgroundColor: colorScheme.surfaceContainer,
                  foregroundColor: colorScheme.onSurfaceVariant,
                ),
                icon: const Icon(Icons.clear_all),
                onPressed: onClearDrawing,
              ),
            ],
            const SizedBox(width: AppSpacing.xs + 2),
            const VerticalDivider(
                width: 1,
                thickness: 1,
                indent: AppSpacing.s,
                endIndent: AppSpacing.s),
            const SizedBox(width: AppSpacing.xs + 2),
            AppButton.tonal(
              text: l10n.next,
              onPressed: isSelectionValid ? onNext : null,
            ),
          ],
        ),
      ),
    );
  }
}
