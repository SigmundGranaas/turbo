import 'dart:io';
import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';
import 'package:turbo/core/widgets/exclusive_sheet.dart';
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

/// Two distinct gestures:
///   * Tap — pan once to the current fix. Doesn't change snap state.
///   * Long-press — sheet with two explicit toggles:
///     - "Snap to my location" (follow mode). When on, the map sticks to
///       your fix until you drag. When off, the map is free.
///     - "Record a path" (Start / Stop).
///
/// The icon still reflects the current snap state so the user can see at a
/// glance which mode the map is in.
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
    final followMode = ref.watch(followModeProvider);
    final colorScheme = Theme.of(context).colorScheme;

    final IconData icon;
    final Color iconColor;
    switch (followMode) {
      case FollowMode.active:
        icon = Icons.my_location;
        iconColor = colorScheme.onTertiaryContainer;
      case FollowMode.paused:
        icon = Icons.location_searching;
        iconColor = colorScheme.primary;
      case FollowMode.off:
        icon = Icons.location_on;
        iconColor = colorScheme.primary;
    }

    return MapControlButtonBase(
      onPressed: _onTap,
      onLongPress: () => _showLocationSheet(context),
      isActive: followMode == FollowMode.active,
      child: Icon(icon, color: iconColor),
    );
  }

  /// Tap behavior:
  ///   * If paused (was following, user dragged) — resume snapping and re-pan.
  ///   * Otherwise — pan once to the current fix without touching state.
  /// The explicit enable/disable toggle lives in the long-press sheet.
  Future<void> _onTap() async {
    if (!kIsWeb && Platform.isLinux) {
      _showLinuxDialog();
      return;
    }
    final wasPaused = ref.read(followModeProvider) == FollowMode.paused;
    if (wasPaused) {
      ref.read(followModeProvider.notifier).resume();
    }
    await _panToCurrentLocation();
  }

  Future<void> _panToCurrentLocation() async {
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
      }
    } catch (error) {
      if (mounted) {
        _showErrorDialog(_translateLocationError(context, error.toString()));
      }
    }
  }

  void _showLocationSheet(BuildContext context) {
    showExclusiveSheet(
      context,
      builder: (BuildContext context) {
        return Consumer(
          builder: (context, ref, _) {
            final followMode = ref.watch(followModeProvider);
            final isRecording =
                ref.watch(recordingNotifierProvider).isActive;
            final colorScheme = Theme.of(context).colorScheme;
            // Paused counts as "on enough" for the switch — the user opted
            // in. They can resume by tapping the location button or fully
            // exit via this switch / the mode chip's close.
            final switchOn = followMode.isOnOrPaused;
            return SafeArea(
              child: Padding(
                padding: const EdgeInsets.symmetric(vertical: 8),
                child: Column(
                  mainAxisSize: MainAxisSize.min,
                  children: [
                    SwitchListTile(
                      secondary: Icon(
                        followMode == FollowMode.active
                            ? Icons.my_location
                            : Icons.location_searching,
                      ),
                      title: const Text('Snap to my location'),
                      subtitle: Text(
                        followMode == FollowMode.paused
                            ? 'Paused — tap the location button to resume.'
                            : 'Keeps the map centered on your position.',
                      ),
                      value: switchOn,
                      onChanged: (next) {
                        if (next) {
                          ref.read(followModeProvider.notifier).enable();
                          _panAfterEnablingFollow();
                        } else {
                          ref.read(followModeProvider.notifier).disable();
                        }
                      },
                    ),
                    ListTile(
                      leading: Icon(
                        isRecording
                            ? Icons.stop_circle
                            : Icons.fiber_manual_record,
                        color: isRecording ? colorScheme.error : null,
                      ),
                      title: Text(
                        isRecording
                            ? 'Stop recording'
                            : 'Record a path',
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

  /// When the user flips the snap switch on, we politely pan to the cached
  /// fix (or wait for one) so the toggle has immediate visible effect.
  /// Failure quietly disables follow so the icon doesn't lie.
  Future<void> _panAfterEnablingFollow() async {
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
        ref.read(followModeProvider.notifier).disable();
      }
    } catch (_) {
      if (mounted) ref.read(followModeProvider.notifier).disable();
    }
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
    await showExclusiveSheet<bool>(
      context,
      builder: (_) => SavePathSheet.fromGeoPath(result.toGeoPath()),
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
