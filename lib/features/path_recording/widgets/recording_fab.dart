import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../data/recording_notifier.dart';
import '../models/recording_state.dart';
import 'permission_dialogs.dart';
import 'recording_controls.dart';

/// Entry point for the recording feature on the main map. Idle → asks for
/// permission and starts a session. Active → opens the controls sheet.
///
/// Hide this when the measuring tool is active to avoid two record-style
/// UIs competing for attention.
class RecordingFab extends ConsumerWidget {
  const RecordingFab({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final rec = ref.watch(recordingNotifierProvider);
    final theme = Theme.of(context);
    final isActive = rec.isActive;
    final isPaused = rec.status == RecordingStatus.paused;

    return FloatingActionButton(
      heroTag: 'recording_fab',
      backgroundColor: isActive
          ? (isPaused ? Colors.amber.shade700 : theme.colorScheme.error)
          : theme.colorScheme.primary,
      foregroundColor: theme.colorScheme.onPrimary,
      tooltip: isActive ? 'Recording controls' : 'Record a path',
      onPressed: () => _onPressed(context, ref),
      child: Icon(
        isActive
            ? (isPaused ? Icons.pause : Icons.stop)
            : Icons.fiber_manual_record,
      ),
    );
  }

  Future<void> _onPressed(BuildContext context, WidgetRef ref) async {
    final notifier = ref.read(recordingNotifierProvider.notifier);
    final current = ref.read(recordingNotifierProvider);

    if (current.isActive) {
      await showModalBottomSheet<void>(
        context: context,
        useSafeArea: true,
        builder: (_) => const RecordingControlsSheet(),
      );
      return;
    }

    final result = await requestRecordingPermissions(context);
    if (result == RecordingPermissionResult.denied) {
      if (context.mounted) {
        ScaffoldMessenger.maybeOf(context)?.showSnackBar(
          const SnackBar(
            content: Text('Location permission is required to record a path.'),
          ),
        );
      }
      return;
    }

    await notifier.start();
    if (!context.mounted) return;
    ScaffoldMessenger.maybeOf(context)?.showSnackBar(
      SnackBar(
        content: Text(
          result == RecordingPermissionResult.foregroundOnly
              ? 'Recording started — keep Turkart in the foreground.'
              : 'Recording started.',
        ),
        duration: const Duration(seconds: 2),
      ),
    );
  }
}
