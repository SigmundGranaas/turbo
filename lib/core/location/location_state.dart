import 'dart:async';
import 'dart:io';

import 'package:flutter/foundation.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:geolocator/geolocator.dart';
import 'package:latlong2/latlong.dart';
import 'package:turbo/core/service/logger.dart';

/// Lightweight snapshot of the latest GPS fix. Decouples downstream features
/// from the geolocator package so they don't all import platform plugins.
@immutable
class PositionSnapshot {
  final LatLng latLng;

  /// Speed over ground in meters/second, if reported by the platform.
  final double? speedMps;

  /// Course over ground in degrees true (0–360), if reported by the platform.
  /// Note: the platform typically only reports this once the device is moving.
  final double? courseDeg;

  /// Horizontal accuracy in meters, if reported.
  final double? accuracyM;

  /// Altitude in meters above the WGS-84 ellipsoid, if reported.
  final double? altitudeM;

  const PositionSnapshot({
    required this.latLng,
    this.speedMps,
    this.courseDeg,
    this.accuracyM,
    this.altitudeM,
  });
}

/// Holds the most recent [PositionSnapshot] from the GPS stream. Updated
/// in-place by [LocationState] so a single platform subscription serves both
/// the lat/lng-only consumers and HUD/marine widgets that need speed/course.
final lastPositionProvider =
    NotifierProvider<LastPositionNotifier, PositionSnapshot?>(
  LastPositionNotifier.new,
);

class LastPositionNotifier extends Notifier<PositionSnapshot?> {
  @override
  PositionSnapshot? build() => null;

  void update(PositionSnapshot? snapshot) {
    state = snapshot;
  }
}

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
          log.warning('Location initialization timed out.');
          return null;
        },
      );
    } catch (e, st) {
      log.warning('Error in LocationState.build', e, st);
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
      _publishSnapshot(position);

      // Start stream for updates
      _positionSubscription = Geolocator.getPositionStream(
        locationSettings: const LocationSettings(
          accuracy: LocationAccuracy.high,
          distanceFilter: 10,
        ),
      ).listen(
            (Position position) {
          state = AsyncData(LatLng(position.latitude, position.longitude));
          _publishSnapshot(position);
        },
        onError: (error, stackTrace) {
          log.warning('Location stream error', error, stackTrace);
        },
      );

      return LatLng(position.latitude, position.longitude);
    } catch (e, st) {
      log.warning('Location setup failed', e, st);
      return null;
    }
  }

  void _publishSnapshot(Position position) {
    // Geolocator reports 0 for unknown fields on some platforms; filter the
    // most common "unset" sentinels so HUDs don't display a confident 0.
    final speed = position.speed;
    final heading = position.heading;
    ref.read(lastPositionProvider.notifier).update(PositionSnapshot(
          latLng: LatLng(position.latitude, position.longitude),
          speedMps: speed.isFinite && speed >= 0 ? speed : null,
          courseDeg:
              heading.isFinite && heading >= 0 && heading <= 360 ? heading : null,
          accuracyM: position.accuracy.isFinite ? position.accuracy : null,
          altitudeM: position.altitude.isFinite ? position.altitude : null,
        ));
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
