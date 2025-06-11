import 'dart:io';
import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:geolocator/geolocator.dart';
import 'package:latlong2/latlong.dart';
import 'package:map_app/widgets/map/controller/map_utility.dart';
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
          _showErrorDialog("Could not determine location.");
        }
      }
    } catch (error) {
      if (mounted) {
        _showErrorDialog(error.toString());
      }
      ref.read(locationStateProvider.notifier).requestLocationPermission();
    }
  }

  void _showLinuxDialog() {
    if (!mounted) return;
    showDialog(
      context: context,
      builder: (BuildContext context) {
        return AlertDialog(
          title: const Text('Location Services Unavailable'),
          content: const Text(
            'Location services are not yet supported on Linux. '
                'We apologize for the inconvenience.',
          ),
          actions: <Widget>[
            TextButton(
              child: const Text('OK'),
              onPressed: () => Navigator.of(context).pop(),
            ),
          ],
        );
      },
    );
  }

  void _showErrorDialog(String message) {
    if (!mounted) return;
    showDialog(
      context: context,
      builder: (BuildContext context) {
        return AlertDialog(
          title: const Text('Location Error'),
          content: Text(message),
          actions: <Widget>[
            TextButton(
              child: const Text('OK'),
              onPressed: () => Navigator.of(context).pop(),
            ),
            TextButton(
              child: const Text('Open Settings'),
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