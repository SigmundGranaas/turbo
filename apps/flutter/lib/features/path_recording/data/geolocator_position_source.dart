import 'dart:async';
import 'dart:io';

import 'package:flutter/foundation.dart' show kIsWeb;
import 'package:geolocator/geolocator.dart';
import 'package:latlong2/latlong.dart';
import 'package:logging/logging.dart';
import 'package:turbo/core/location/gps_accuracy_mode.dart';

import '../models/recording_sample.dart';
import 'position_source.dart';

final _log = Logger('GeolocatorPositionSource');

/// Production [PositionSource] backed by `geolocator`. On Android, runs
/// inside a foreground service so the stream survives screen-off and the
/// app being backgrounded; on iOS, requests `allowsBackgroundLocationUpdates`
/// via the plist key. On web, the stream is foreground-only — the browser
/// will pause it when the tab is hidden.
class GeolocatorPositionSource implements PositionSource {
  StreamSubscription<Position>? _subscription;
  StreamController<RecordingSample>? _controller;

  @override
  Stream<RecordingSample> stream(GpsAccuracyMode mode) {
    final controller = StreamController<RecordingSample>.broadcast(
      onListen: () => _start(mode),
      onCancel: () => _stop(),
    );
    _controller = controller;
    return controller.stream;
  }

  Future<void> _start(GpsAccuracyMode mode) async {
    try {
      final settings = _settingsFor(mode);
      _subscription = Geolocator.getPositionStream(locationSettings: settings)
          .listen(
        (pos) {
          _controller?.add(
            RecordingSample(
              position: LatLng(pos.latitude, pos.longitude),
              timestamp: pos.timestamp,
              elevation: pos.altitude == 0 && pos.altitudeAccuracy == 0
                  ? null
                  : pos.altitude,
              accuracyMeters: pos.accuracy,
              speedMetersPerSecond: pos.speed,
            ),
          );
        },
        onError: (Object error, StackTrace st) {
          _log.warning('Recording position stream error: $error');
          _controller?.addError(error, st);
        },
      );
    } catch (e, st) {
      _log.warning('Failed to start position stream: $e');
      _controller?.addError(e, st);
    }
  }

  Future<void> _stop() async {
    await _subscription?.cancel();
    _subscription = null;
  }

  LocationSettings _settingsFor(GpsAccuracyMode mode) {
    final accuracy = switch (mode) {
      GpsAccuracyMode.high => LocationAccuracy.best,
      GpsAccuracyMode.balanced => LocationAccuracy.high,
      GpsAccuracyMode.batterySaver => LocationAccuracy.medium,
    };
    final filter = switch (mode) {
      GpsAccuracyMode.high => 0,
      GpsAccuracyMode.balanced => 5,
      GpsAccuracyMode.batterySaver => 15,
    };

    if (kIsWeb) {
      return LocationSettings(accuracy: accuracy, distanceFilter: filter);
    }
    if (Platform.isAndroid) {
      return AndroidSettings(
        accuracy: accuracy,
        distanceFilter: filter,
        forceLocationManager: false,
        intervalDuration: const Duration(seconds: 1),
        foregroundNotificationConfig: const ForegroundNotificationConfig(
          notificationTitle: 'Recording your hike',
          notificationText: 'Turkart is tracking your route in the background.',
          enableWakeLock: true,
        ),
      );
    }
    if (Platform.isIOS || Platform.isMacOS) {
      return AppleSettings(
        accuracy: accuracy,
        distanceFilter: filter,
        activityType: ActivityType.fitness,
        pauseLocationUpdatesAutomatically: false,
        showBackgroundLocationIndicator: true,
        allowBackgroundLocationUpdates: true,
      );
    }
    return LocationSettings(accuracy: accuracy, distanceFilter: filter);
  }

  @override
  Future<void> dispose() async {
    await _subscription?.cancel();
    _subscription = null;
    await _controller?.close();
    _controller = null;
  }
}
