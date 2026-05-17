import 'dart:async';

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:latlong2/latlong.dart';
import 'package:shared_preferences/shared_preferences.dart';
import 'package:sqflite_common_ffi/sqflite_ffi.dart';
import 'package:turbo/core/data/database_provider.dart';
import 'package:turbo/features/path_recording/api.dart';
import 'package:turbo/features/saved_paths/api.dart';
import 'package:turbo/features/saved_paths/data/sqlite_saved_path_datastore.dart';
import 'package:turbo/features/settings/api.dart';

import '../../helpers/in_memory_db.dart';
import '../../helpers/pump_app.dart';
import '../../helpers/wait_for.dart';

class _FakePositionSource implements PositionSource {
  final StreamController<RecordingSample> controller =
      StreamController<RecordingSample>.broadcast();

  @override
  Stream<RecordingSample> stream(GpsAccuracyMode mode) => controller.stream;

  @override
  Future<void> dispose() async {
    await controller.close();
  }
}

void main() {
  late Database db;
  late _FakePositionSource fakeSource;
  late ProviderContainer container;

  setUpAll(() {
    initSqfliteFfi();
  });

  setUp(() async {
    SharedPreferences.setMockInitialValues({});
    db = await createSavedPathsDb();
    fakeSource = _FakePositionSource();
    container = ProviderContainer(overrides: [
      databaseProvider.overrideWith((ref) async => db),
      positionSourceProvider.overrideWithValue(fakeSource),
    ]);
    container.listen(savedPathRepositoryProvider, (_, _) {});
    container.listen(settingsProvider, (_, _) {});
    await waitForAsyncData(container, savedPathRepositoryProvider);
  });

  tearDown(() async {
    container.dispose();
    await fakeSource.dispose();
    await db.close();
  });

  test(
      'full flow: record → stop → save via repository → reload from SQLite',
      () async {
    final notifier = container.read(recordingNotifierProvider.notifier);
    await notifier.start();

    final t0 = DateTime(2026, 5, 17, 10);
    final altitudes = <double>[
      1000, 1010, 1020, 1030, 1040, 1050,
      1040, 1030, 1020, 1010, 1000,
    ];
    for (var i = 0; i < altitudes.length; i++) {
      fakeSource.controller.add(RecordingSample(
        position: LatLng(61.50 + i * 0.001, 8.75 + i * 0.001),
        timestamp: t0.add(Duration(seconds: i * 2)),
        elevation: altitudes[i],
      ));
    }
    await Future<void>.delayed(const Duration(milliseconds: 100));

    final result = await notifier.stop();
    expect(result, isNotNull);
    expect(result!.points.length, altitudes.length);

    // Build the SavedPath the same way SavePathSheet does and hand it to the
    // repository — this exercises serialization, persistence and reload.
    final elevationsForSave =
        result.elevations.map((e) => e ?? double.nan).toList(growable: false);
    final repo = container.read(savedPathRepositoryProvider.notifier);
    final newPath = SavedPath(
      uuid: 'recorded-hike',
      title: 'My Hike',
      points: result.points,
      distance: result.distanceMeters,
      elevations: elevationsForSave,
      recordedAt: result.recordedAt,
      ascent: result.ascent,
      descent: result.descent,
      movingTimeSeconds: result.movingTimeSeconds,
    );
    await repo.addPath(newPath);

    final store = SQLiteSavedPathDataStore(db);
    final persisted = await store.getByUuid('recorded-hike');
    expect(persisted, isNotNull);
    expect(persisted!.title, 'My Hike');
    expect(persisted.points.length, altitudes.length);
    expect(persisted.elevations, isNotNull);
    expect(persisted.elevations!.length, altitudes.length);
    expect(persisted.elevations!.first, 1000.0);
    expect(persisted.elevations!.last, 1000.0);
    expect(persisted.recordedAt, isNotNull);
    expect(persisted.ascent, greaterThan(0));
    expect(persisted.descent, greaterThan(0));
  });

  test('M1 round-trip: recorded path survives SQLite save and load', () async {
    final store = SQLiteSavedPathDataStore(db);
    final path = SavedPath(
      uuid: 'recorded-1',
      title: 'Round Trip',
      points: const [LatLng(60, 10), LatLng(60.01, 10.01), LatLng(60.02, 10.02)],
      distance: 2500,
      elevations: const [1100.0, 1120.0, 1110.0],
      recordedAt: DateTime.utc(2026, 5, 17, 14, 30),
      ascent: 20.0,
      descent: 10.0,
      movingTimeSeconds: 1800,
    );
    await store.insert(path);

    final loaded = await store.getByUuid('recorded-1');
    expect(loaded, isNotNull);
    expect(loaded!.elevations, [1100.0, 1120.0, 1110.0]);
    expect(loaded.recordedAt, DateTime.utc(2026, 5, 17, 14, 30));
    expect(loaded.ascent, 20.0);
    expect(loaded.descent, 10.0);
    expect(loaded.movingTimeSeconds, 1800);
  });

  test('null elevations round-trip (paths from before M1 stay readable)',
      () async {
    final store = SQLiteSavedPathDataStore(db);
    final path = SavedPath(
      uuid: 'pre-m1',
      title: 'No Elevation',
      points: const [LatLng(60, 10), LatLng(60.01, 10.01)],
      distance: 1500,
    );
    await store.insert(path);

    final loaded = await store.getByUuid('pre-m1');
    expect(loaded!.elevations, isNull);
    expect(loaded.recordedAt, isNull);
    expect(loaded.ascent, isNull);
  });
}
