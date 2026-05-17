import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:turbo/features/settings/api.dart';

import '../data/recording_notifier.dart';
import '../models/recording_state.dart';

/// Translucent band rendered at the top of the map while a recording is in
/// flight. Shows live distance, duration, and current pace. The widget is
/// hidden entirely when there is no active recording.
class RecordingHud extends ConsumerStatefulWidget {
  const RecordingHud({super.key});

  @override
  ConsumerState<RecordingHud> createState() => _RecordingHudState();
}

class _RecordingHudState extends ConsumerState<RecordingHud> {
  Timer? _ticker;

  @override
  void initState() {
    super.initState();
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
    final unit =
        ref.watch(settingsProvider).value?.distanceUnit ?? DistanceUnit.metric;
    final isPaused = rec.status == RecordingStatus.paused;
    final accent = isPaused ? Colors.amber.shade700 : theme.colorScheme.primary;

    final duration = rec.startedAt == null
        ? Duration.zero
        : DateTime.now().difference(rec.startedAt!);

    return SafeArea(
      child: Padding(
        padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 8),
        child: Material(
          color: theme.colorScheme.surface.withValues(alpha: 0.92),
          elevation: 4,
          borderRadius: BorderRadius.circular(14),
          child: Padding(
            padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 10),
            child: Row(
              mainAxisSize: MainAxisSize.min,
              children: [
                Icon(
                  isPaused ? Icons.pause_circle_filled : Icons.fiber_manual_record,
                  color: accent,
                  size: 18,
                ),
                const SizedBox(width: 10),
                _HudStat(
                  label: 'Distance',
                  value: formatDistance(rec.distanceMeters, unit),
                ),
                const SizedBox(width: 18),
                _HudStat(label: 'Time', value: _formatDuration(duration)),
                const SizedBox(width: 18),
                _HudStat(
                  label: 'Elev gain',
                  value: '${rec.ascent.round()} m',
                ),
              ],
            ),
          ),
        ),
      ),
    );
  }

  String _formatDuration(Duration d) {
    final h = d.inHours;
    final m = d.inMinutes.remainder(60).toString().padLeft(2, '0');
    final s = d.inSeconds.remainder(60).toString().padLeft(2, '0');
    if (h > 0) return '$h:$m:$s';
    return '$m:$s';
  }
}

class _HudStat extends StatelessWidget {
  final String label;
  final String value;

  const _HudStat({required this.label, required this.value});

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return Column(
      mainAxisSize: MainAxisSize.min,
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Text(
          label.toUpperCase(),
          style: theme.textTheme.labelSmall?.copyWith(
            color: theme.colorScheme.onSurfaceVariant,
            letterSpacing: 0.6,
          ),
        ),
        Text(value, style: theme.textTheme.titleMedium),
      ],
    );
  }
}
