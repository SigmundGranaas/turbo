import 'dart:async';

import 'package:riverpod_annotation/riverpod_annotation.dart';
import 'package:geolocator/geolocator.dart';
import 'package:latlong2/latlong.dart';
import 'package:flutter/foundation.dart';
import 'dart:io';

part 'location_state.g.dart';

@riverpod
class LocationState extends _$LocationState {
  StreamSubscription<Position>? _positionSubscription;

  @override
  AsyncValue<LatLng?> build() {
    _initLocationTracking();

    // Dispose of the subscription when the provider is disposed
    ref.onDispose(() {
      _positionSubscription?.cancel();
    });

    return const AsyncValue.loading();
  }

  Future<void> _initLocationTracking() async {
    if (!kIsWeb && Platform.isLinux) {
      state = const AsyncValue.data(null);
      return;
    }

    try {
      final serviceEnabled = await Geolocator.isLocationServiceEnabled();
      if (!serviceEnabled) {
        state = const AsyncValue.error(
          'Location services are disabled',
          StackTrace.empty,
        );
        return;
      }

      var permission = await Geolocator.checkPermission();
      if (permission == LocationPermission.denied) {
        permission = await Geolocator.requestPermission();
        if (permission == LocationPermission.denied) {
          state = const AsyncValue.error(
            'Location permissions are denied',
            StackTrace.empty,
          );
          return;
        }
      }

      if (permission == LocationPermission.deniedForever) {
        state = const AsyncValue.error(
          'Location permissions are permanently denied',
          StackTrace.empty,
        );
        return;
      }

      // Start listening to location updates
      _positionSubscription = Geolocator.getPositionStream(
        locationSettings: const LocationSettings(
          accuracy: LocationAccuracy.high,
          distanceFilter: 10,
        ),
      ).listen(
            (Position position) {
          state = AsyncValue.data(
            LatLng(position.latitude, position.longitude),
          );
        },
        onError: (error) {
          state = AsyncValue.error(error, StackTrace.current);
        },
      );
    } catch (error, stack) {
      state = AsyncValue.error(error, stack);
    }
  }

  Future<void> requestLocationPermission() async {
    await _initLocationTracking();
  }

  Future<AsyncValue<LatLng?>> position() async {
    return state;
  }
}

// Optional provider for location settings
@riverpod
class LocationSettingsProv extends _$LocationSettingsProv{
  @override
  LocationAccuracy build() => LocationAccuracy.high;

  void setAccuracy(LocationAccuracy accuracy) {
    state = accuracy;
  }
}
