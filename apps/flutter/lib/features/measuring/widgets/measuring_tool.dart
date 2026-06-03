import 'package:flutter/material.dart' hide CatmullRomSpline;
import 'package:flutter_map/flutter_map.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import 'package:turbo/app/l10n/app_localizations.dart';
import 'package:turbo/core/util/catmull_rom_spline.dart';
import 'package:turbo/core/widgets/app_snackbars.dart';
import 'package:turbo/core/widgets/dismissible_tool_sheet.dart';
import 'package:turbo/core/widgets/exclusive_sheet.dart';
import 'package:turbo/core/widgets/map/buttons/map_control_button_base.dart';
import 'package:turbo/features/map_view/api.dart';
import 'package:turbo/features/saved_paths/api.dart';
import 'package:turbo/features/settings/api.dart';

import '../data/measuring_state.dart';
import '../data/measuring_state_notifier.dart';
import '../data/measure_geo_path.dart';
import '../models/measure_point_type.dart';
import 'measuring_controls.dart';
import 'measuring_line.dart';
import 'measuring_markers.dart';

const String measuringToolId = 'measuring';

/// Measuring as an in-place [MapToolDescriptor] mounted on the single shared
/// map — replaces the old full-screen `MeasuringMapPage` with its own
/// `MapController`. State lives in [measuringStateProvider] so the layers,
/// overlay, taps and freehand pointer stream all share it.
final measuringStateProvider =
    NotifierProvider.autoDispose<MeasuringStateNotifier, MeasuringState>(
  MeasuringStateNotifier.new,
);

final measuringTool = MapToolDescriptor(
  id: measuringToolId,
  buildLayers: (ctx) => [const MeasureToolLayers()],
  buildOverlay: (ctx) => const MeasureToolOverlay(),
  onMapTap: (ctx, point) {
    if (!ctx.ref.read(measuringStateProvider).isDrawing) {
      ctx.ref.read(measuringStateProvider.notifier).addPoint(point);
    }
  },
  onPointerDown: (ctx, e, p) {
    if (ctx.ref.read(measuringStateProvider).isDrawing) {
      ctx.ref.read(measuringStateProvider.notifier).handlePointerDown(e, p);
    }
  },
  onPointerMove: (ctx, e, p) {
    if (ctx.ref.read(measuringStateProvider).isDrawing) {
      ctx.ref.read(measuringStateProvider.notifier).handlePointerMove(e, p);
    }
  },
  onPointerUp: (ctx, e, p) {
    if (ctx.ref.read(measuringStateProvider).isDrawing) {
      ctx.ref.read(measuringStateProvider.notifier).handlePointerUp(e, p);
    }
  },
  interaction: (ctx) {
    final drawing = ctx.ref.watch(measuringStateProvider).isDrawing;
    return InteractionOptions(
      flags: drawing
          ? (InteractiveFlag.all & ~InteractiveFlag.drag & ~InteractiveFlag.rotate)
          : (InteractiveFlag.all & ~InteractiveFlag.rotate),
      pinchZoomThreshold: 0.2,
      pinchMoveThreshold: 40,
    );
  },
  onDeactivate: (ctx) => ctx.ref.read(measuringStateProvider.notifier).reset(),
);

class MeasureToolLayers extends ConsumerWidget {
  const MeasureToolLayers({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final state = ref.watch(measuringStateProvider);
    final settings = ref.watch(settingsProvider).value;
    final isSmoothing = settings?.smoothLine ?? true;
    final showIntermediate = settings?.showIntermediatePoints ?? false;

    final markerPoints = showIntermediate
        ? state.points
        : state.points
            .where((p) =>
                p.type == MeasurePointType.start ||
                p.type == MeasurePointType.end)
            .toList();

    final raw = state.points.map((p) => p.point).toList();
    final line =
        isSmoothing ? CatmullRomSpline(controlPoints: raw).generate() : raw;

    return Stack(
      children: [
        MeasurePolyline(points: line),
        MeasureMarkers(points: markerPoints),
      ],
    );
  }
}

class MeasureToolOverlay extends ConsumerWidget {
  const MeasureToolOverlay({super.key});

  Future<void> _finish(BuildContext context, WidgetRef ref) async {
    final state = ref.read(measuringStateProvider);
    if (state.points.length < 2) {
      AppSnackbars.info(context, context.l10n.needMorePoints);
      return;
    }
    final isSmoothing = ref.read(settingsProvider).value?.smoothLine ?? true;
    final geoPath = measurePointsToGeoPath(
      state.points,
      distanceM: state.totalDistance,
    );
    final saved = await showExclusiveSheet<bool>(
      context,
      builder: (_) =>
          SavePathSheet.fromGeoPath(geoPath, isSmoothing: isSmoothing),
    );
    if (saved == true && context.mounted) {
      AppSnackbars.success(context, context.l10n.pathSaved);
    }
    if (saved != null) {
      ref.read(activeMapToolProvider.notifier).deactivate();
    }
  }

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final state = ref.watch(measuringStateProvider);
    final notifier = ref.read(measuringStateProvider.notifier);
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
          left: 16,
          right: 16,
          bottom: 24,
          child: SafeArea(
            top: false,
            child: Center(
              child: DismissibleToolSheet(
                onDismiss: () =>
                    ref.read(activeMapToolProvider.notifier).deactivate(),
                child: MeasuringControls(
                  distance: state.totalDistance,
                  onReset: notifier.reset,
                  onUndo: notifier.undoLastPoint,
                  onFinish: () => _finish(context, ref),
                  onToggleDrawing: notifier.toggleDrawing,
                  canUndo: state.points.isNotEmpty,
                  canReset: state.points.isNotEmpty,
                  canSave: state.points.length >= 2,
                  isDrawing: state.isDrawing,
                ),
              ),
            ),
          ),
        ),
      ],
    );
  }
}
