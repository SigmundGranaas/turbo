import 'dart:io';
import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:geolocator/geolocator.dart';
import 'package:latlong2/latlong.dart';
import 'package:map_app/widgets/map/controller/map_utility.dart';

import '../../../data/state/providers/location_state.dart';
import '../controller/provider/map_controller.dart';

class LocationButton extends ConsumerStatefulWidget {
  const LocationButton({super.key});

  @override
  ConsumerState<LocationButton> createState() => LocationButtonState();
}

class LocationButtonState extends ConsumerState<LocationButton> with TickerProviderStateMixin {

  @override
  Widget build(BuildContext context) {
    return Card(
      elevation: 4,
      child: Padding(
        padding: const EdgeInsets.all(8.0),
        child: IconButton(
          icon: const Icon(Icons.location_on),
          onPressed: () => _moveToCurrentLocation(context, ref),
        ),
      ),
    );
  }

  Future<void> _moveToCurrentLocation(BuildContext context, WidgetRef ref) async {
    if (!kIsWeb && Platform.isLinux) {
      _showLinuxDialog(context);
      return;
    }

    try {
      final position = await _getCurrentPosition(context, ref);
      if(position != null){
        final controller = ref.read(mapControllerProvProvider.notifier).controller();
        animatedMapMove(LatLng(position.latitude, position.longitude), 15, controller, this);
      }

        } catch (error) {
      if (context.mounted) {
        _showErrorDialog(context, error.toString());
      }
    }
  }

  Future<LatLng?> _getCurrentPosition(BuildContext context,  WidgetRef ref) async {
    final future = await ref.read(locationStateProvider.notifier).position();
    return future.value;
  }

  void _showLinuxDialog(BuildContext context) {
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

  void _showErrorDialog(BuildContext context, String message) {
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