import 'dart:io';
import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:geolocator/geolocator.dart';
import 'package:turbo/l10n/app_localizations.dart';
import 'package:turbo/widgets/map/controller/map_utility.dart';
import 'package:flutter_map/flutter_map.dart';

import '../../../data/state/providers/location_state.dart';
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
    return MapControlButtonBase(
      onPressed: () => _moveToCurrentLocation(),
      child: Icon(Icons.location_on, color: Theme.of(context).colorScheme.primary),
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
      final position = await ref.read(locationStateProvider.future);
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