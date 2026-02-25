import 'dart:async';
import 'dart:io';

import 'package:flutter/foundation.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:geolocator/geolocator.dart';
import 'package:latlong2/latlong.dart';

final locationStateProvider =
AsyncNotifierProvider.autoDispose<LocationState, LatLng?>(
  LocationState.new,
);

class LocationState extends AsyncNotifier<LatLng?> {
  StreamSubscription<Position>? _positionSubscription;

  @override
  Future<LatLng?> build() async {
    ref.onDispose(() {
      _positionSubscription?.cancel();
    });

    // Start listening but don't wait for the first value in build if it might hang.
    // However, build() must return the initial value or wait for it.
    // We'll use a timeout to prevent infinite hangs on problematic emulators.
    try {
      return await _setupLocationListener().timeout(
        const Duration(seconds: 5),
        onTimeout: () {
          debugPrint("Location initialization timed out.");
          return null;
        },
      );
    } catch (e) {
      debugPrint("Error in LocationState.build: $e");
      return null;
    }
  }

  Future<LatLng?> _setupLocationListener() async {
    if (!kIsWeb && Platform.isLinux) {
      return null;
    }

    try {
      final serviceEnabled = await Geolocator.isLocationServiceEnabled();
      if (!serviceEnabled) {
        return null;
      }

      var permission = await Geolocator.checkPermission();
      if (permission == LocationPermission.denied) {
        permission = await Geolocator.requestPermission();
        if (permission == LocationPermission.denied) {
          return null;
        }
      }

      if (permission == LocationPermission.deniedForever) {
        return null;
      }

      // Get initial position
      final position = await Geolocator.getCurrentPosition(
        locationSettings: const LocationSettings(
          accuracy: LocationAccuracy.high,
        ),
      );

      // Start stream for updates
      _positionSubscription = Geolocator.getPositionStream(
        locationSettings: const LocationSettings(
          accuracy: LocationAccuracy.high,
          distanceFilter: 10,
        ),
      ).listen(
            (Position position) {
          state = AsyncData(LatLng(position.latitude, position.longitude));
        },
        onError: (error, stackTrace) {
          debugPrint("Location stream error: $error");
        },
      );

      return LatLng(position.latitude, position.longitude);
    } catch (e) {
      debugPrint("Location setup failed: $e");
      return null;
    }
  }

  Future<void> requestLocationPermission() async {
    ref.invalidateSelf();
    await future;
  }
}

final locationSettingsProvProvider =
NotifierProvider.autoDispose<LocationSettingsProv, LocationAccuracy>(
  LocationSettingsProv.new,
);

class LocationSettingsProv extends Notifier<LocationAccuracy> {
  @override
  LocationAccuracy build() => LocationAccuracy.high;

  void setAccuracy(LocationAccuracy accuracy) {
    state = accuracy;
  }
}