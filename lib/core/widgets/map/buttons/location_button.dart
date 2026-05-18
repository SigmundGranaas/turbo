import 'dart:io';
import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:geolocator/geolocator.dart';
import 'package:turbo/app/l10n/app_localizations.dart';
import 'package:turbo/core/widgets/map/controller/map_utility.dart';
import 'package:flutter_map/flutter_map.dart';

import 'package:turbo/core/location/follow_mode_state.dart';
import 'package:turbo/core/location/location_state.dart';
import 'package:turbo/features/path_recording/api.dart';
import 'package:turbo/features/saved_paths/api.dart' show SavePathSheet;
import 'map_control_button_base.dart';

/// Single primary action: toggle "snap the map to my location" mode.
///   * Off (icon `location_on`): map is free — drag anywhere.
///   * On  (icon `my_location`): map snaps to your fix and keeps tracking
///     it until you drag (manual drag automatically disengages — handled
///     in `MainMapPage._onMapEvent`).
///
/// Long-press opens recording controls (Start / Stop). Recording is the
/// only thing on the long-press sheet now — Follow lives on the tap, so
/// the two concepts aren't fighting for the same gesture.
class LocationButton extends ConsumerStatefulWidget {
  final MapController mapController;
  const LocationButton({super.key, required this.mapController});

  @override
  ConsumerState<LocationButton> createState() => LocationButtonState();
}

class LocationButtonState extends ConsumerState<LocationButton>
    with TickerProviderStateMixin {
  @override
  Widget build(BuildContext context) {
    final isFollowing = ref.watch(followModeProvider);
    final colorScheme = Theme.of(context).colorScheme;

    return MapControlButtonBase(
      onPressed: _toggleFollow,
      onLongPress: () => _showRecordingSheet(context),
      isActive: isFollowing,
      child: Icon(
        isFollowing ? Icons.my_location : Icons.location_on,
        color: isFollowing
            ? colorScheme.onTertiaryContainer
            : colorScheme.primary,
      ),
    );
  }

  /// Tap behavior: toggle follow mode. Enabling also pans the camera to the
  /// current fix so the user gets immediate feedback that snapping is on.
  Future<void> _toggleFollow() async {
    if (!kIsWeb && Platform.isLinux) {
      _showLinuxDialog();
      return;
    }
    final wasFollowing = ref.read(followModeProvider);
    ref.read(followModeProvider.notifier).toggle();
    if (wasFollowing) return; // Just turned off — nothing more to do.

    // Just turned on — pan to the user. If a fix is already cached, the pan
    // is instant; otherwise we wait for the provider to resolve.
    final cached = ref.read(locationStateProvider).value;
    if (cached != null) {
      animatedMapMove(cached, 15, widget.mapController, this);
      return;
    }
    final l10n = context.l10n;
    try {
      ref.read(locationStateProvider.notifier).requestLocationPermission();
      final position = await ref.read(locationStateProvider.future);
      if (!mounted) return;
      if (position != null) {
        animatedMapMove(position, 15, widget.mapController, this);
      } else {
        ScaffoldMessenger.maybeOf(context)?.showSnackBar(SnackBar(
          content: Text(l10n.locationServicesUnavailable),
          duration: const Duration(seconds: 3),
        ));
        // Couldn't actually snap — back out of follow mode so the icon
        // doesn't lie about the current state.
        ref.read(followModeProvider.notifier).disable();
      }
    } catch (error) {
      if (mounted) {
        _showErrorDialog(_translateLocationError(context, error.toString()));
      }
      if (mounted) ref.read(followModeProvider.notifier).disable();
    }
  }

  void _showRecordingSheet(BuildContext context) {
    showModalBottomSheet(
      context: context,
      useSafeArea: true,
      builder: (BuildContext context) {
        return Consumer(
          builder: (context, ref, _) {
            final isRecording =
                ref.watch(recordingNotifierProvider).isActive;
            return SafeArea(
              child: Padding(
                padding: const EdgeInsets.all(16),
                child: Column(
                  mainAxisSize: MainAxisSize.min,
                  children: [
                    ListTile(
                      leading: Icon(
                        isRecording
                            ? Icons.stop_circle
                            : Icons.fiber_manual_record,
                        color: isRecording
                            ? Theme.of(context).colorScheme.error
                            : null,
                      ),
                      title: Text(
                        isRecording
                            ? 'Stop recording'
                            : 'Start recording a path',
                      ),
                      subtitle: isRecording
                          ? const Text('Tap to save your track.')
                          : const Text('Or long-press the location marker.'),
                      onTap: () async {
                        Navigator.pop(context);
                        if (isRecording) {
                          await _stopRecordingAndSave(context, ref);
                        } else {
                          await startRecordingFlow(context, ref);
                        }
                      },
                    ),
                  ],
                ),
              ),
            );
          },
        );
      },
    );
  }

  Future<void> _stopRecordingAndSave(
      BuildContext context, WidgetRef ref) async {
    final result =
        await ref.read(recordingNotifierProvider.notifier).stop();
    if (!context.mounted) return;
    if (result == null || result.isEmpty) {
      ScaffoldMessenger.maybeOf(context)?.showSnackBar(
        const SnackBar(content: Text('No track to save — not enough fixes.')),
      );
      return;
    }
    await showModalBottomSheet<bool>(
      context: context,
      isScrollControlled: true,
      useSafeArea: true,
      builder: (_) => SavePathSheet(
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

  String _translateLocationError(BuildContext context, String error) {
    final l10n = context.l10n;
    final key = error.replaceFirst('Exception: ', '').trim();
    switch (key) {
      case 'location_services_disabled':
        return l10n.locationServicesDisabled;
      case 'location_permissions_denied':
        return l10n.locationPermissionsDenied;
      case 'location_permissions_denied_forever':
        return l10n.locationPermissionsDeniedForever;
      default:
        return error;
    }
  }

  void _showLinuxDialog() {
    if (!mounted) return;
    final l10n = context.l10n;
    showDialog(
      context: context,
      builder: (BuildContext context) {
        return AlertDialog(
          title: Text(l10n.locationServicesUnavailable),
          content: Text(l10n.locationServicesUnsupportedOnPlatform),
          actions: <Widget>[
            TextButton(
              child: Text(l10n.ok),
              onPressed: () => Navigator.of(context).pop(),
            ),
          ],
        );
      },
    );
  }

  void _showErrorDialog(String message) {
    if (!mounted) return;
    final l10n = context.l10n;
    showDialog(
      context: context,
      builder: (BuildContext context) {
        return AlertDialog(
          title: Text(l10n.locationError),
          content: Text(message),
          actions: <Widget>[
            TextButton(
              child: Text(l10n.ok),
              onPressed: () => Navigator.of(context).pop(),
            ),
            TextButton(
              child: Text(l10n.openSettings),
              onPressed: () {
                Navigator.of(context).pop();
                Geolocator.openLocationSettings();
              },
            ),
          ],
        );
      },
    );
  }
}
