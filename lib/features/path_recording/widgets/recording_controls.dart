import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:turbo/core/widgets/app_button.dart';
import 'package:turbo/features/saved_paths/api.dart';

import '../data/recording_notifier.dart';
import '../models/recording_state.dart';

/// Bottom-sheet controls for an in-flight recording: pause/resume, stop
/// (which opens [SavePathSheet] with the captured data), and discard.
///
/// Surface via `showModalBottomSheet(... RecordingControlsSheet())`.
class RecordingControlsSheet extends ConsumerWidget {
  const RecordingControlsSheet({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final rec = ref.watch(recordingNotifierProvider);
    final theme = Theme.of(context);
    final mediaQuery = MediaQuery.of(context);
    final bottomPadding = mediaQuery.padding.bottom;

    final isPaused = rec.status == RecordingStatus.paused;
    final isIdle = rec.status == RecordingStatus.idle;

    return Padding(
      padding: EdgeInsets.fromLTRB(16, 16, 16, 16 + bottomPadding),
      child: Column(
        mainAxisSize: MainAxisSize.min,
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          Row(
            mainAxisAlignment: MainAxisAlignment.spaceBetween,
            children: [
              Text(
                isIdle
                    ? 'No active recording'
                    : (isPaused ? 'Recording paused' : 'Recording'),
                style: theme.textTheme.titleLarge,
              ),
              IconButton(
                onPressed: () => Navigator.of(context).pop(),
                icon: const Icon(Icons.close),
              ),
            ],
          ),
          const SizedBox(height: 16),
          if (isIdle)
            const Text('Start a recording from the map screen.')
          else ...[
            AppButton.primary(
              text: isPaused ? 'Resume' : 'Pause',
              onPressed: () async {
                final notifier = ref.read(recordingNotifierProvider.notifier);
                if (isPaused) {
                  await notifier.resume();
                } else {
                  notifier.pause();
                }
              },
              fullWidth: true,
              icon: isPaused ? Icons.play_arrow : Icons.pause,
            ),
            const SizedBox(height: 12),
            AppButton.primary(
              text: 'Stop & save',
              onPressed: () => _stopAndSave(context, ref),
              fullWidth: true,
              icon: Icons.stop,
            ),
            const SizedBox(height: 12),
            AppButton.secondary(
              text: 'Discard',
              onPressed: () => _confirmDiscard(context, ref),
              fullWidth: true,
            ),
          ],
        ],
      ),
    );
  }

  Future<void> _stopAndSave(BuildContext context, WidgetRef ref) async {
    final navigator = Navigator.of(context);
    final messenger = ScaffoldMessenger.maybeOf(context);
    final result =
        await ref.read(recordingNotifierProvider.notifier).stop();
    navigator.pop();

    if (result == null || result.isEmpty) {
      messenger?.showSnackBar(
        const SnackBar(content: Text('No track to save — not enough fixes.')),
      );
      return;
    }

    if (!context.mounted) return;
    await showModalBottomSheet<bool>(
      context: context,
      isScrollControlled: true,
      useSafeArea: true,
      builder: (sheetCtx) => SavePathSheet(
        points: result.points,
        distance: result.distanceMeters,
        elevations: result.elevations,
        recordedAt: result.recordedAt,
        ascent: result.ascent,
        descent: result.descent,
        movingTimeSeconds: result.movingTimeSeconds,
      ),
    );
  }

  Future<void> _confirmDiscard(BuildContext context, WidgetRef ref) async {
    final discard = await showDialog<bool>(
      context: context,
      builder: (ctx) => AlertDialog(
        title: const Text('Discard recording?'),
        content: const Text('This will throw away the track you just recorded.'),
        actions: [
          TextButton(
            onPressed: () => Navigator.of(ctx).pop(false),
            child: const Text('Keep'),
          ),
          FilledButton(
            onPressed: () => Navigator.of(ctx).pop(true),
            child: const Text('Discard'),
          ),
        ],
      ),
    );
    if (discard == true) {
      await ref.read(recordingNotifierProvider.notifier).discard();
      if (context.mounted) Navigator.of(context).pop();
    }
  }
}
