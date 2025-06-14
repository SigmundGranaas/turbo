import 'dart:async';
import 'dart:io';

import 'package:flutter/foundation.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:geolocator/geolocator.dart';
import 'package:latlong2/latlong.dart';

final locationStateProvider =
AutoDisposeAsyncNotifierProvider<LocationState, LatLng?>(
  LocationState.new,
);

class LocationState extends AutoDisposeAsyncNotifier<LatLng?> {
  StreamSubscription<Position>? _positionSubscription;

  @override
  Future<LatLng?> build() async {
    final completer = Completer<LatLng?>();

    ref.onDispose(() {
      _positionSubscription?.cancel();
    });

    _setupLocationListener(completer);

    return completer.future;
  }

  Future<void> _setupLocationListener(Completer<LatLng?> completer) async {
    try {
      if (!kIsWeb && Platform.isLinux) {
        if (!completer.isCompleted) completer.complete(null);
        return;
      }

      final serviceEnabled = await Geolocator.isLocationServiceEnabled();
      if (!serviceEnabled) {
        throw Exception('location_services_disabled');
      }

      var permission = await Geolocator.checkPermission();
      if (permission == LocationPermission.denied) {
        permission = await Geolocator.requestPermission();
        if (permission == LocationPermission.denied) {
          throw Exception('location_permissions_denied');
        }
      }

      if (permission == LocationPermission.deniedForever) {
        throw Exception('location_permissions_denied_forever');
      }

      _positionSubscription = Geolocator.getPositionStream(
        locationSettings: const LocationSettings(
          accuracy: LocationAccuracy.high,
          distanceFilter: 10,
        ),
      ).listen(
            (Position position) {
          final newPosition = LatLng(position.latitude, position.longitude);
          if (!completer.isCompleted) {
            completer.complete(newPosition);
          }
          state = AsyncData(newPosition);
        },
        onError: (error, stackTrace) {
          if (!completer.isCompleted) {
            completer.completeError(error, stackTrace);
          }
          state = AsyncError(error, stackTrace);
        },
      );
    } catch (e, st) {
      if (!completer.isCompleted) {
        completer.completeError(e, st);
      }
      state = AsyncError(e, st);
    }
  }

  Future<void> requestLocationPermission() async {
    ref.invalidateSelf();
    await future;
  }
}

final locationSettingsProvProvider =
AutoDisposeNotifierProvider<LocationSettingsProv, LocationAccuracy>(
  LocationSettingsProv.new,
);

class LocationSettingsProv extends AutoDisposeNotifier<LocationAccuracy> {
  @override
  LocationAccuracy build() => LocationAccuracy.high;

  void setAccuracy(LocationAccuracy accuracy) {
    state = accuracy;
  }
}