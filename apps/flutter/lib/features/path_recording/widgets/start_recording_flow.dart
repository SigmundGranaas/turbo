import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:turbo/core/widgets/app_snackbars.dart';

import '../data/recording_notifier.dart';
import '../models/recording_state.dart';
import 'permission_dialogs.dart';

/// Imperative entry point for starting a recording. Used both by the
/// long-press gesture on the current-location marker and any other surface
/// that wants to kick off a recording. Handles:
///
/// * No-op when one is already in flight (we just hint via snackbar).
/// * Two-stage permission prompt via [requestRecordingPermissions].
/// * Friendly toast on success / denial.
///
/// The actual UI for an active recording lives in `RecordingPanel`, mounted
/// once on the main map page; this helper does NOT show controls.
Future<void> startRecordingFlow(BuildContext context, WidgetRef ref) async {
  final messenger = ScaffoldMessenger.maybeOf(context);
  final current = ref.read(recordingNotifierProvider);
  if (current.status != RecordingStatus.idle) {
    messenger?.showSnackBar(const SnackBar(
      content: Text('A recording is already running.'),
    ));
    return;
  }

  final result = await requestRecordingPermissions(context, ref);
  if (result == RecordingPermissionResult.denied) {
    if (context.mounted) {
      AppSnackbars.error(
          context, 'Location permission is required to record a path.');
    }
    return;
  }

  await ref.read(recordingNotifierProvider.notifier).start();
  if (!context.mounted) return;
  AppSnackbars.info(
    context,
    result == RecordingPermissionResult.foregroundOnly
        ? 'Recording started — keep Turkart in the foreground.'
        : 'Recording started.',
  );
}
