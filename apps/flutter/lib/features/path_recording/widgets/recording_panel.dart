import 'dart:async';

import 'package:flutter/material.dart';
import 'package:turbo/core/widgets/exclusive_sheet.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:turbo/app/tokens.dart';
import 'package:turbo/core/widgets/app_button.dart';
import 'package:turbo/core/widgets/app_dialog.dart';
import 'package:turbo/core/widgets/app_pill.dart';
import 'package:turbo/core/widgets/app_snackbars.dart';
import 'package:turbo/features/saved_paths/api.dart';
import 'package:turbo/features/settings/api.dart';

import '../data/recording_notifier.dart';
import '../models/recording_geo_path.dart';
import '../models/recording_state.dart';

/// Bottom-anchored control panel shown while a recording is in flight.
/// Mirrors the layout language of `MeasuringControls`: stat row + tonal save
/// CTA, divider, then a row of secondary actions. Hidden when status == idle.
class RecordingPanel extends ConsumerStatefulWidget {
  const RecordingPanel({super.key});

  @override
  ConsumerState<RecordingPanel> createState() => _RecordingPanelState();
}

class _RecordingPanelState extends ConsumerState<RecordingPanel> {
  Timer? _ticker;

  @override
  void initState() {
    super.initState();
    // Drives the live duration display. The recorder itself emits state on
    // every fix, but we want the clock to tick even when no fix arrives.
    _ticker = Timer.periodic(const Duration(seconds: 1), (_) {
      if (mounted) setState(() {});
    });
  }

  @override
  void dispose() {
    _ticker?.cancel();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final rec = ref.watch(recordingNotifierProvider);
    if (!rec.isActive) return const SizedBox.shrink();

    final theme = Theme.of(context);
    final colorScheme = theme.colorScheme;
    final textTheme = theme.textTheme;
    final unit = ref.watch(settingsProvider
        .select((s) => s.value?.distanceUnit ?? DistanceUnit.metric));
    final isPaused = rec.status == RecordingStatus.paused;
    final accentColor =
        isPaused ? colorScheme.tertiary : colorScheme.error;
    final canSave = rec.points.length >= 2;

    final duration = rec.startedAt == null
        ? Duration.zero
        : DateTime.now().difference(rec.startedAt!);

    return AppCardSurface(
      margin: const EdgeInsets.symmetric(horizontal: AppSpacing.l),
      maxWidth: 700,
      child: Column(
        mainAxisSize: MainAxisSize.min,
        children: [
          // Top row: status dot + stat grid + Save CTA.
          Padding(
            padding: const EdgeInsets.fromLTRB(
                AppSpacing.l, AppSpacing.m, AppSpacing.l, AppSpacing.m),
            child: Row(
              crossAxisAlignment: CrossAxisAlignment.center,
              children: [
                _StatusDot(color: accentColor, paused: isPaused),
                const SizedBox(width: AppSpacing.m),
                Expanded(
                  child: Row(
                    mainAxisSize: MainAxisSize.min,
                    children: [
                      Expanded(
                        child: _Stat(
                          label: 'Distance',
                          value: formatDistance(rec.distanceMeters, unit),
                          textTheme: textTheme,
                          colorScheme: colorScheme,
                        ),
                      ),
                      Expanded(
                        child: _Stat(
                          label: 'Time',
                          value: _formatDuration(duration),
                          textTheme: textTheme,
                          colorScheme: colorScheme,
                        ),
                      ),
                      Expanded(
                        child: _Stat(
                          label: 'Elev gain',
                          value: _formatElevation(rec.ascent, unit),
                          textTheme: textTheme,
                          colorScheme: colorScheme,
                        ),
                      ),
                    ],
                  ),
                ),
                const SizedBox(width: AppSpacing.s),
                AppButton.tonal(
                  text: 'Save',
                  onPressed: canSave ? () => _stopAndSave(context) : null,
                ),
              ],
            ),
          ),
          const Divider(
              height: 1, indent: AppSpacing.l, endIndent: AppSpacing.l),
          // Bottom row: pause/resume + discard.
          Padding(
            padding: const EdgeInsets.symmetric(
                horizontal: AppSpacing.m, vertical: AppSpacing.xs),
            child: Row(
              children: [
                IconButton(
                  onPressed: isPaused
                      ? ref.read(recordingNotifierProvider.notifier).resume
                      : ref.read(recordingNotifierProvider.notifier).pause,
                  icon: Icon(isPaused ? Icons.play_arrow : Icons.pause),
                  tooltip: isPaused ? 'Resume' : 'Pause',
                ),
                const Spacer(),
                IconButton(
                  onPressed: () => _confirmDiscard(context),
                  icon: const Icon(Icons.delete_sweep_outlined),
                  tooltip: 'Discard recording',
                ),
              ],
            ),
          ),
        ],
      ),
    );
  }

  Future<void> _stopAndSave(BuildContext context) async {
    final messenger = ScaffoldMessenger.maybeOf(context);
    final result =
        await ref.read(recordingNotifierProvider.notifier).stop();
    if (result == null || result.isEmpty) {
      messenger?.showSnackBar(const SnackBar(
        content: Text('No track to save — not enough fixes.'),
      ));
      return;
    }
    if (!context.mounted) return;
    await showExclusiveSheet<bool>(
      context,
      builder: (_) => SavePathSheet.fromGeoPath(result.toGeoPath()),
    );
  }

  Future<void> _confirmDiscard(BuildContext context) async {
    final confirmed = await AppDialog.destructive(
      context,
      title: 'Discard recording?',
      content: 'This will throw away the track you just recorded.',
      destructiveLabel: 'Discard',
    );
    if (!confirmed) return;
    await ref.read(recordingNotifierProvider.notifier).discard();
    if (context.mounted) {
      AppSnackbars.info(context, 'Recording discarded.');
    }
  }

  String _formatDuration(Duration d) {
    final h = d.inHours;
    final m = d.inMinutes.remainder(60).toString().padLeft(2, '0');
    final s = d.inSeconds.remainder(60).toString().padLeft(2, '0');
    if (h > 0) return '$h:$m:$s';
    return '$m:$s';
  }

  String _formatElevation(double meters, DistanceUnit unit) {
    if (unit == DistanceUnit.imperial) {
      return '${(meters / 0.3048).round()} ft';
    }
    return '${meters.round()} m';
  }
}

class _Stat extends StatelessWidget {
  final String label;
  final String value;
  final TextTheme textTheme;
  final ColorScheme colorScheme;

  const _Stat({
    required this.label,
    required this.value,
    required this.textTheme,
    required this.colorScheme,
  });

  @override
  Widget build(BuildContext context) {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      mainAxisSize: MainAxisSize.min,
      children: [
        Text(
          label,
          style: textTheme.bodySmall?.copyWith(
            color: colorScheme.onSurfaceVariant,
          ),
        ),
        Text(
          value,
          style: textTheme.titleMedium?.copyWith(
            fontWeight: FontWeight.bold,
            color: colorScheme.onSurface,
          ),
        ),
      ],
    );
  }
}

class _StatusDot extends StatefulWidget {
  final Color color;
  final bool paused;
  const _StatusDot({required this.color, required this.paused});

  @override
  State<_StatusDot> createState() => _StatusDotState();
}

class _StatusDotState extends State<_StatusDot>
    with SingleTickerProviderStateMixin {
  late final AnimationController _controller;

  @override
  void initState() {
    super.initState();
    _controller = AnimationController(
      duration: const Duration(milliseconds: 1100),
      vsync: this,
    )..repeat(reverse: true);
  }

  @override
  void dispose() {
    _controller.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    if (widget.paused) {
      return Icon(Icons.pause_circle_filled, color: widget.color, size: 22);
    }
    return AnimatedBuilder(
      animation: _controller,
      builder: (_, _) => Container(
        width: 14,
        height: 14,
        decoration: BoxDecoration(
          shape: BoxShape.circle,
          color: widget.color.withValues(alpha: 0.5 + 0.5 * _controller.value),
        ),
      ),
    );
  }
}
