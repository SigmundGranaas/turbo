import 'dart:io';

import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';
import 'package:flutter_map/flutter_map.dart';
import 'package:geolocator/geolocator.dart';
import 'package:latlong2/latlong.dart';

class LocationButton extends StatelessWidget {
  final MapController mapController;

  const LocationButton({super.key, required this.mapController});

  Future<void> _getCurrentLocation(BuildContext context) async {
    if (!kIsWeb && Platform.isLinux) {
      // Display a prompt for Linux users
      showDialog(
        context: context,
        builder: (BuildContext context) {
          return AlertDialog(
            title: const Text('Location Services Unavailable'),
            content: const Text('Location services are not yet supported on Linux. We apologize for the inconvenience.'),
            actions: <Widget>[
              TextButton(
                child: const Text('OK'),
                onPressed: () {
                  Navigator.of(context).pop();
                },
              ),
            ],
          );
        },
      );
      return;
    }

    // Original Geolocator code for supported platforms
    bool serviceEnabled = await Geolocator.isLocationServiceEnabled();
    if (!serviceEnabled) {
      return Future.error('Location services are disabled.');
    }

    LocationPermission permission = await Geolocator.checkPermission();
    if (permission == LocationPermission.denied) {
      permission = await Geolocator.requestPermission();
      if (permission == LocationPermission.denied) {
        return Future.error('Location permissions are denied');
      }
    }

    if (permission == LocationPermission.deniedForever) {
      return Future.error('Location permissions are permanently denied, we cannot request permissions.');
    }

    try {
      Position position = await Geolocator.getCurrentPosition(
          desiredAccuracy: LocationAccuracy.high
      );

      mapController.move(LatLng(position.latitude, position.longitude), 15.0);
    } catch (e) {
      print("Error getting location: $e");
    }
  }

  @override
  Widget build(BuildContext context) {
    return Card(
      elevation: 4,
      child: Padding(
        padding: const EdgeInsets.all(8.0),
        child:IconButton(
            icon: const Icon(Icons.location_on),
            onPressed: () => _getCurrentLocation(context)
        ),
      ),
    );
  }
}