import 'dart:io';
import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:latlong2/latlong.dart';
import 'package:geolocator/geolocator.dart';
import 'package:turbo/l10n/app_localizations.dart';
import 'package:turbo/core/widgets/map/controller/map_utility.dart';
import 'package:flutter_map/flutter_map.dart';

import 'package:turbo/core/location/follow_mode_state.dart';
import 'package:turbo/core/location/location_state.dart';
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
      onPressed: () => _moveToCurrentLocation(),
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
                  ],
                ),
              ),
            );
          },
        );
      },
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

    try {
      final LatLng? position = await ref.read(locationStateProvider.future);
      if (position != null) {
        animatedMapMove(position, 15, widget.mapController, this);
      } else {
        if (mounted) {
          _showErrorDialog(context.l10n.locationServicesUnavailable);
        }
      }
    } catch (error) {
      if (mounted) {
        _showErrorDialog(_translateLocationError(context, error.toString()));
      }
      ref.read(locationStateProvider.notifier).requestLocationPermission();
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
