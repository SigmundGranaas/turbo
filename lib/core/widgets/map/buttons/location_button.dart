import 'dart:io';
import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:latlong2/latlong.dart';
import 'package:geolocator/geolocator.dart';
import 'package:turbo/app/l10n/app_localizations.dart';
import 'package:turbo/core/widgets/map/controller/map_utility.dart';
import 'package:flutter_map/flutter_map.dart';

import 'package:turbo/core/location/follow_mode_state.dart';
import 'package:turbo/core/location/location_state.dart';
import 'package:turbo/features/path_recording/api.dart';
import 'package:turbo/features/saved_paths/api.dart' show SavePathSheet;
import 'map_control_button_base.dart';

class LocationButton extends ConsumerStatefulWidget {
  final MapController mapController;
  const LocationButton({super.key, required this.mapController});

  @override
  ConsumerState<LocationButton> createState() => LocationButtonState();
}

class LocationButtonState extends ConsumerState<LocationButton> with TickerProviderStateMixin {
  @override
  Widget build(BuildContext context) {
    final isFollowing = ref.watch(followModeProvider);
    final colorScheme = Theme.of(context).colorScheme;

    return MapControlButtonBase(
      onPressed: _moveToCurrentLocation,
      onLongPress: () => _showLocationSheet(context),
      isActive: isFollowing,
      child: Icon(
        isFollowing ? Icons.my_location : Icons.location_on,
        color: isFollowing
            ? colorScheme.onTertiaryContainer
            : colorScheme.primary,
      ),
    );
  }

  void _showLocationSheet(BuildContext context) {
    final l10n = context.l10n;
    showModalBottomSheet(
      context: context,
      useSafeArea: true,
      builder: (BuildContext context) {
        return Consumer(
          builder: (context, ref, _) {
            final isFollowing = ref.watch(followModeProvider);
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
                        isFollowing ? Icons.location_off : Icons.my_location,
                      ),
                      title: Text(
                        isFollowing
                            ? l10n.stopFollowing
                            : l10n.followMyLocation,
                      ),
                      onTap: () {
                        Navigator.pop(context);
                        ref.read(followModeProvider.notifier).toggle();
                        if (!isFollowing) {
                          _moveToCurrentLocation();
                        }
                      },
                    ),
                    ListTile(
                      leading: Icon(
                        isRecording ? Icons.stop_circle : Icons.fiber_manual_record,
                        color: isRecording
                            ? Theme.of(context).colorScheme.error
                            : null,
                      ),
                      title: Text(
                        isRecording ? 'Stop recording' : 'Start recording a path',
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

  Future<void> _stopRecordingAndSave(BuildContext context, WidgetRef ref) async {
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

  Future<void> _moveToCurrentLocation() async {
    if (!kIsWeb && Platform.isLinux) {
      _showLinuxDialog();
      return;
    }
    final l10n = context.l10n;

    // Fast path: if we already have a recent fix, just pan to it.
    final cached = ref.read(locationStateProvider).value;
    if (cached != null) {
      animatedMapMove(cached, 15, widget.mapController, this);
      return;
    }

    try {
      // If we got an error from the previous run, retry — this also covers
      // the "permissions just granted from system prompt" case.
      ref.read(locationStateProvider.notifier).requestLocationPermission();
      final LatLng? position = await ref.read(locationStateProvider.future);
      if (!mounted) return;
      if (position != null) {
        animatedMapMove(position, 15, widget.mapController, this);
      } else {
        // Loaded with no error and no position — likely services off or the
        // OS still warming up the GPS. A friendly snackbar is less jarring
        // than an alert dialog; the user can tap again in a second.
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
