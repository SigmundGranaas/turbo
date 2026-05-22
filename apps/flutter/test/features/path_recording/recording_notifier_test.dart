import 'dart:async';

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:latlong2/latlong.dart';
import 'package:shared_preferences/shared_preferences.dart';
import 'package:turbo/features/path_recording/api.dart';
import 'package:turbo/features/settings/api.dart';

class _FakePositionSource implements PositionSource {
  final StreamController<RecordingSample> controller =
      StreamController<RecordingSample>.broadcast();

  GpsAccuracyMode? lastMode;

  @override
  Stream<RecordingSample> stream(GpsAccuracyMode mode) {
    lastMode = mode;
    return controller.stream;
  }

  @override
  Future<void> dispose() async {
    await controller.close();
  }
}

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();
  SharedPreferences.setMockInitialValues({});

  late _FakePositionSource fake;
  late ProviderContainer container;

  setUp(() {
    fake = _FakePositionSource();
    container = ProviderContainer(overrides: [
      positionSourceProvider.overrideWithValue(fake),
    ]);
    // Force settings to seed so recording_notifier finds a SettingsState
    // and doesn't fall back to defaults silently.
    container.listen(settingsProvider, (_, _) {});
  });

  tearDown(() async {
    container.dispose();
    await fake.dispose();
  });

  Future<void> waitForSettings() async {
    for (var i = 0; i < 50; i++) {
      if (container.read(settingsProvider).value != null) return;
      await Future.delayed(const Duration(milliseconds: 10));
    }
    fail('Settings did not initialize');
  }

  test('start → samples accumulate → stop returns a RecordingResult', () async {
    await waitForSettings();

    final notifier = container.read(recordingNotifierProvider.notifier);
    await notifier.start();

    expect(
      container.read(recordingNotifierProvider).status,
      RecordingStatus.recording,
    );

    final t0 = DateTime(2026, 5, 17, 10, 0, 0);
    // Climb 1000 → 1050 over 6 fixes, then descend back to 1000.
    final altitudes = [1000.0, 1010.0, 1020.0, 1030.0, 1040.0, 1050.0,
                       1040.0, 1030.0, 1020.0, 1010.0, 1000.0];
    for (var i = 0; i < altitudes.length; i++) {
      fake.controller.add(RecordingSample(
        position: LatLng(61.50 + i * 0.001, 8.75 + i * 0.001),
        timestamp: t0.add(Duration(seconds: i * 2)),
        elevation: altitudes[i],
      ));
    }

    // Give the broadcast stream a tick to deliver.
    await Future.delayed(const Duration(milliseconds: 50));

    final result = await notifier.stop();
    expect(result, isNotNull);
    expect(result!.points.length, altitudes.length);
    expect(result.distanceMeters, greaterThan(0));
    expect(result.elevations.first, 1000.0);
    expect(result.elevations.last, 1000.0);
    // We climbed 50 m and descended 50 m — both well above the 1 m noise floor.
    expect(result.ascent, greaterThan(20));
    expect(result.descent, greaterThan(20));

    // State resets to idle after stop().
    expect(
      container.read(recordingNotifierProvider).status,
      RecordingStatus.idle,
    );
  });

  test('pause drops in-flight samples; resume keeps prior data', () async {
    await waitForSettings();
    final notifier = container.read(recordingNotifierProvider.notifier);
    await notifier.start();

    final t0 = DateTime(2026, 5, 17);
    fake.controller.add(RecordingSample(
      position: const LatLng(60.0, 10.0),
      timestamp: t0,
      elevation: 100,
    ));
    await Future.delayed(const Duration(milliseconds: 30));

    notifier.pause();
    expect(
      container.read(recordingNotifierProvider).status,
      RecordingStatus.paused,
    );

    // A sample delivered while paused should not be accumulated.
    fake.controller.add(RecordingSample(
      position: const LatLng(60.5, 10.5),
      timestamp: t0.add(const Duration(seconds: 5)),
      elevation: 200,
    ));
    await Future.delayed(const Duration(milliseconds: 30));

    await notifier.resume();
    fake.controller.add(RecordingSample(
      position: const LatLng(60.001, 10.001),
      timestamp: t0.add(const Duration(seconds: 10)),
      elevation: 105,
    ));
    await Future.delayed(const Duration(milliseconds: 30));

    final result = await notifier.stop();
    expect(result, isNotNull);
    expect(result!.points.length, 2);
    expect(result.points.first.latitude, closeTo(60.0, 0.0001));
    expect(result.points.last.latitude, closeTo(60.001, 0.0001));
  });

  test('stop with fewer than 2 fixes returns null', () async {
    await waitForSettings();
    final notifier = container.read(recordingNotifierProvider.notifier);
    await notifier.start();
    fake.controller.add(RecordingSample(
      position: const LatLng(0, 0),
      timestamp: DateTime.now(),
      elevation: 0,
    ));
    await Future.delayed(const Duration(milliseconds: 30));
    final result = await notifier.stop();
    expect(result, isNull);
  });

  test('discard throws away the session without producing a result', () async {
    await waitForSettings();
    final notifier = container.read(recordingNotifierProvider.notifier);
    await notifier.start();
    fake.controller.add(RecordingSample(
      position: const LatLng(0, 0),
      timestamp: DateTime.now(),
      elevation: 0,
    ));
    fake.controller.add(RecordingSample(
      position: const LatLng(0.001, 0.001),
      timestamp: DateTime.now().add(const Duration(seconds: 1)),
      elevation: 0,
    ));
    await Future.delayed(const Duration(milliseconds: 30));
    await notifier.discard();
    expect(
      container.read(recordingNotifierProvider).status,
      RecordingStatus.idle,
    );
    expect(container.read(recordingNotifierProvider).points, isEmpty);
  });

  test('GPS accuracy mode from settings is passed to the position source',
      () async {
    await waitForSettings();
    await container
        .read(settingsProvider.notifier)
        .setGpsAccuracyMode(GpsAccuracyMode.batterySaver);
    await container.read(recordingNotifierProvider.notifier).start();
    expect(fake.lastMode, GpsAccuracyMode.batterySaver);
  });

  test('low-accuracy samples are dropped (only when accuracy is reported)',
      () async {
    await waitForSettings();
    final notifier = container.read(recordingNotifierProvider.notifier);
    await notifier.start();

    final t0 = DateTime(2026, 5, 17);
    // Good fix → accepted.
    fake.controller.add(RecordingSample(
      position: const LatLng(61.50000, 8.75000),
      timestamp: t0,
      elevation: 1000,
      accuracyMeters: 5,
    ));
    // Bad fix (40 m accuracy, mode is high → 25 m cap) → dropped.
    fake.controller.add(RecordingSample(
      position: const LatLng(61.55000, 8.85000),
      timestamp: t0.add(const Duration(seconds: 1)),
      elevation: 1010,
      accuracyMeters: 40,
    ));
    // Another good fix, ~5m further along — well under the 50 m/s ceiling
    // so the jump filter leaves it alone.
    fake.controller.add(RecordingSample(
      position: const LatLng(61.50005, 8.75005),
      timestamp: t0.add(const Duration(seconds: 2)),
      elevation: 1005,
      accuracyMeters: 8,
    ));
    await Future.delayed(const Duration(milliseconds: 50));

    final result = await notifier.stop();
    expect(result, isNotNull);
    expect(result!.points.length, 2,
        reason: 'low-accuracy middle sample should be dropped');
    expect(result.points.first, const LatLng(61.50000, 8.75000));
    expect(result.points.last, const LatLng(61.50005, 8.75005));
  });

  test('teleport-speed jumps are rejected when accuracy is reported',
      () async {
    await waitForSettings();
    final notifier = container.read(recordingNotifierProvider.notifier);
    await notifier.start();

    final t0 = DateTime(2026, 5, 17);
    fake.controller.add(RecordingSample(
      position: const LatLng(61.50, 8.75),
      timestamp: t0,
      accuracyMeters: 5,
    ));
    // 1 km in 1 second = 1000 m/s, well above the 50 m/s ceiling.
    fake.controller.add(RecordingSample(
      position: const LatLng(61.51, 8.75),
      timestamp: t0.add(const Duration(seconds: 1)),
      accuracyMeters: 5,
    ));
    fake.controller.add(RecordingSample(
      position: const LatLng(61.5001, 8.7501),
      timestamp: t0.add(const Duration(seconds: 2)),
      accuracyMeters: 5,
    ));
    await Future.delayed(const Duration(milliseconds: 50));

    final result = await notifier.stop();
    expect(result!.points.length, 2,
        reason: 'teleport middle sample should be dropped');
  });
}
