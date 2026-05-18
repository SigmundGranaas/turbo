import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';
import 'package:geolocator/geolocator.dart';

/// Outcome of the runtime permission request flow. Foreground means we can
/// record while the app is visible; background unlocks lock-screen and
/// app-backgrounded recording. Denied means recording is not possible at all.
enum RecordingPermissionResult { granted, foregroundOnly, denied }

/// Two-stage permission flow:
///   1. Request "while in use" — required to record at all.
///   2. If foreground granted, ask the user whether they want background
///      recording, then request `always`.
///
/// The rationale dialogs use plain English copy; production code can swap
/// these for l10n strings without changing the call sites.
Future<RecordingPermissionResult> requestRecordingPermissions(
  BuildContext context,
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

  // Foreground granted; offer to upgrade.
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
  );
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
}) {
  return showDialog<bool>(
    context: context,
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
