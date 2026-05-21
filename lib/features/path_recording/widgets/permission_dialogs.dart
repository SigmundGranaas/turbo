import 'dart:async';

import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:geolocator/geolocator.dart';
import 'package:turbo/features/settings/api.dart';

/// Outcome of the runtime permission request flow. Foreground means we can
/// record while the app is visible; background unlocks lock-screen and
/// app-backgrounded recording. Denied means recording is not possible at all.
enum RecordingPermissionResult { granted, foregroundOnly, denied }

/// Two-stage permission flow:
///   1. Request "while in use" — required to record at all.
///   2. If foreground granted and the user hasn't been asked yet, offer to
///      upgrade to background ("Always"). The choice is persisted so we
///      never block subsequent recording starts with the same dialog.
///
/// The rationale dialogs use plain English copy; production code can swap
/// these for l10n strings without changing the call sites.
Future<RecordingPermissionResult> requestRecordingPermissions(
  BuildContext context,
  WidgetRef ref,
) async {
  final serviceEnabled = await Geolocator.isLocationServiceEnabled();
  if (!serviceEnabled) {
    if (!context.mounted) return RecordingPermissionResult.denied;
    await _showOkDialog(
      context,
      title: 'Location services are off',
      message:
          'Turn on location services in your device settings so Turkart can track your route.',
    );
    return RecordingPermissionResult.denied;
  }

  var current = await Geolocator.checkPermission();
  if (current == LocationPermission.denied) {
    if (!context.mounted) return RecordingPermissionResult.denied;
    final wantsToContinue = await _showConfirmDialog(
      context,
      title: 'Track your hike',
      message:
          'Turkart needs your location to record where you go. We only use '
          'it while a recording is active.',
      confirmLabel: 'Allow',
    );
    if (wantsToContinue != true) return RecordingPermissionResult.denied;
    current = await Geolocator.requestPermission();
  }

  if (current == LocationPermission.denied ||
      current == LocationPermission.deniedForever) {
    return RecordingPermissionResult.denied;
  }

  // Web has no notion of "always" permission. Foreground-only is the ceiling.
  if (kIsWeb) {
    return RecordingPermissionResult.foregroundOnly;
  }

  if (current == LocationPermission.always) {
    return RecordingPermissionResult.granted;
  }

  // Foreground granted; offer to upgrade — but only the first time. If the
  // user has already answered this prompt, respect their previous choice
  // and don't block recording on it again.
  final settings = ref.read(settingsProvider).value;
  if (settings != null && settings.backgroundLocationPromptSeen) {
    return RecordingPermissionResult.foregroundOnly;
  }

  if (!context.mounted) return RecordingPermissionResult.foregroundOnly;
  final wantsBackground = await _showConfirmDialog(
    context,
    title: 'Keep recording in the background?',
    message:
        'For lock-screen recording, allow location "Always" on the next '
        'prompt. Skip this if you only record while the app is open — '
        'we will warn you if a recording is interrupted.',
    confirmLabel: 'Enable background',
    cancelLabel: 'Foreground only',
    barrierDismissible: false,
  );
  // Persist the fact that we've asked. Whatever the user picks (or if they
  // back out), we honour it next time — no nagging.
  unawaited(ref
      .read(settingsProvider.notifier)
      .setBackgroundLocationPromptSeen(true));

  if (wantsBackground != true) {
    return RecordingPermissionResult.foregroundOnly;
  }

  final upgraded = await Geolocator.requestPermission();
  if (upgraded == LocationPermission.always) {
    return RecordingPermissionResult.granted;
  }
  return RecordingPermissionResult.foregroundOnly;
}

Future<bool?> _showConfirmDialog(
  BuildContext context, {
  required String title,
  required String message,
  required String confirmLabel,
  String cancelLabel = 'Not now',
  bool barrierDismissible = true,
}) {
  return showDialog<bool>(
    context: context,
    barrierDismissible: barrierDismissible,
    builder: (ctx) => AlertDialog(
      title: Text(title),
      content: Text(message),
      actions: [
        TextButton(
          onPressed: () => Navigator.of(ctx).pop(false),
          child: Text(cancelLabel),
        ),
        FilledButton(
          onPressed: () => Navigator.of(ctx).pop(true),
          child: Text(confirmLabel),
        ),
      ],
    ),
  );
}

Future<void> _showOkDialog(
  BuildContext context, {
  required String title,
  required String message,
}) {
  return showDialog<void>(
    context: context,
    builder: (ctx) => AlertDialog(
      title: Text(title),
      content: Text(message),
      actions: [
        TextButton(
          onPressed: () => Navigator.of(ctx).pop(),
          child: const Text('OK'),
        ),
      ],
    ),
  );
}
