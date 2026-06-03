import 'dart:io';
import 'dart:typed_data';

import 'package:flutter/material.dart';
import 'package:flutter_localizations/flutter_localizations.dart';
import 'package:flutter_map/flutter_map.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:latlong2/latlong.dart';
import 'package:logging/logging.dart';
import 'package:path/path.dart' as p;
import 'package:shared_preferences/shared_preferences.dart';
import 'package:shelf/shelf.dart' as shelf;
import 'package:shelf/shelf_io.dart' as shelf_io;
import 'package:sqflite_common_ffi/sqflite_ffi.dart';
import 'package:turbo/app/l10n/app_localizations.dart';
import 'package:turbo/core/data/database_provider.dart';
import 'package:turbo/core/service/logger.dart';
import 'package:turbo/features/tile_providers/api.dart';
import 'package:turbo/features/tile_storage/offline_regions/api.dart';
import 'package:turbo/features/tile_storage/offline_regions/data/region_repository.dart';
import 'package:turbo/features/tile_storage/offline_regions/data/tile_job_queue.dart';
import 'package:turbo/features/tile_storage/tile_store/api.dart';

class _ServerBackedProvider extends TileProviderConfig {
  _ServerBackedProvider(this.urlTemplate);
  @override
  final String urlTemplate;
  @override
  String get id => 'test_provider';
  @override
  String name(BuildContext context) => 'Test Tiles';
  @override
  String description(BuildContext context) => '';
  @override
  String get attributions => 'Test';
  @override
  TileProviderCategory get category => TileProviderCategory.global;
  @override
  double get minZoom => 1;
  @override
  double get maxZoom => 2;
}

class _FakeRegistry extends TileRegistry {
  _FakeRegistry(this._urlTemplate);
  final String _urlTemplate;

  @override
  TileRegistryState build() {
    final pcfg = _ServerBackedProvider(_urlTemplate);
    return TileRegistryState(
      availableProviders: {pcfg.id: pcfg},
      activeGlobalIds: [pcfg.id],
      activeLocalIds: const [],
      activeOverlayIds: const [],
      activeOfflineIds: const [],
    );
  }
}

Future<void> _createSchema(Database db, int _) async {
  final batch = db.batch();
  batch.execute('''
    CREATE TABLE offline_regions(
      id TEXT PRIMARY KEY, name TEXT NOT NULL, minLat REAL NOT NULL,
      minLng REAL NOT NULL, maxLat REAL NOT NULL, maxLng REAL NOT NULL,
      minZoom INTEGER NOT NULL, maxZoom INTEGER NOT NULL,
      urlTemplate TEXT NOT NULL, tileProviderId TEXT NOT NULL,
      tileProviderName TEXT NOT NULL, status INTEGER NOT NULL,
      totalTiles INTEGER NOT NULL, downloadedTiles INTEGER NOT NULL,
      createdAt TEXT NOT NULL
    )
  ''');
  batch.execute('''
    CREATE TABLE tile_jobs(
      regionId TEXT NOT NULL, providerId TEXT NOT NULL, z INTEGER NOT NULL,
      x INTEGER NOT NULL, y INTEGER NOT NULL, url TEXT NOT NULL,
      status INTEGER NOT NULL, attemptCount INTEGER NOT NULL DEFAULT 0,
      workerId TEXT, startedAt TEXT, PRIMARY KEY (regionId, z, x, y)
    )
  ''');
  batch.execute('CREATE INDEX idx_job_status ON tile_jobs (status)');
  batch.execute('''
    CREATE TABLE tile_store(
      providerId TEXT NOT NULL, z INTEGER NOT NULL, x INTEGER NOT NULL,
      y INTEGER NOT NULL, path TEXT NOT NULL, sizeInBytes INTEGER NOT NULL,
      lastAccessed TEXT NOT NULL, referenceCount INTEGER NOT NULL DEFAULT 0,
      PRIMARY KEY (providerId, z, x, y)
    )
  ''');
  await batch.commit(noResult: true);
}

void main() {
  // User-story test: tap "Start Download" in the real [DownloadDetailsSheet]
  // and verify the region transitions to DownloadStatus.completed through the
  // real notifier + orchestrator (no fakes for the download pipeline).

  setUpAll(() {
    sqfliteFfiInit();
    databaseFactory = databaseFactoryFfi;
    setupLogging(level: Level.WARNING);
    // flutter_test installs a global HttpOverrides that turns every real
    // network request into an empty 400 response. Disable it so our local
    // shelf tile server is reachable from inside the test.
    HttpOverrides.global = null;
  });

  testWidgets(
      'tapping Start Download persists a region and the orchestrator '
      'drives it to completion against a local tile server', (tester) async {
    late Directory tempDir;
    late Database db;
    late HttpServer server;
    late String urlTemplate;
    late ProviderContainer container;

    final tileBytes = Uint8List.fromList([0xFF, 0xD8, 0xFF, 0xE0]);

    // Real I/O + a SQLite db + an HTTP server have to run outside the
    // FakeAsync clock that testWidgets installs by default.
    await tester.runAsync(() async {
      SharedPreferences.setMockInitialValues({});
      tempDir = await Directory.systemTemp.createTemp('offline_user_flow_');
      db = await databaseFactory.openDatabase(
        p.join(tempDir.path, 'turbo.db'),
        options: OpenDatabaseOptions(version: 1, onCreate: _createSchema),
      );
      server = await shelf_io.serve(
        (shelf.Request _) => shelf.Response.ok(tileBytes,
            headers: {'Content-Type': 'image/png'}),
        InternetAddress.loopbackIPv4,
        0,
      );
      urlTemplate = 'http://127.0.0.1:${server.port}/{z}/{x}/{y}.png';

      container = ProviderContainer(
        overrides: [
          databaseProvider.overrideWith((ref) async => db),
          tileStoreServiceProvider.overrideWith(
              (ref) async => TileStoreService(db, testDirectory: tempDir.path)),
          tileRegistryProvider.overrideWith(() => _FakeRegistry(urlTemplate)),
        ],
      );
      await container.read(tileStoreServiceProvider.future);
      await container.read(regionRepositoryProvider.future);
      await container.read(tileJobQueueProvider.future);
      await container.read(offlineRegionsProvider.future);
      // Mirrors main.dart: a listen() makes the orchestrator start ticking.
      container.listen(downloadOrchestratorProvider, (_, _) {});
    });

    addTearDown(() async {
      await tester.runAsync(() async {
        container.read(downloadOrchestratorProvider)?.stop();
        container.dispose();
        await db.close();
        await server.close(force: true);
        if (await tempDir.exists()) {
          await tempDir.delete(recursive: true);
        }
      });
    });

    final bounds = LatLngBounds(
      const LatLng(-85, -180),
      const LatLng(85, 180),
    );

    await tester.pumpWidget(
      UncontrolledProviderScope(
        container: container,
        child: MaterialApp(
          localizationsDelegates: const [
            AppLocalizations.delegate,
            GlobalMaterialLocalizations.delegate,
            GlobalWidgetsLocalizations.delegate,
            GlobalCupertinoLocalizations.delegate,
          ],
          supportedLocales: AppLocalizations.supportedLocales,
          home: Scaffold(body: DownloadDetailsSheet(bounds: bounds)),
        ),
      ),
    );
    // Let the addPostFrameCallback fill in the default name + initial provider.
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 50));

    expect(find.text('My Offline Map'), findsOneWidget,
        reason: 'default region name should be pre-filled');
    expect(find.text('Test Tiles'), findsWidgets,
        reason: 'test tile provider should be the active selection');

    // Tap "Start Download" — the user-visible commit point.
    await tester.tap(find.text('Start Download'));
    await tester.pump();

    // Wait for the region to appear via createRegion.
    String? regionId;
    await tester.runAsync(() async {
      final deadline = DateTime.now().add(const Duration(seconds: 3));
      while (DateTime.now().isBefore(deadline)) {
        final regions = container.read(offlineRegionsProvider).value ??
            const <OfflineRegion>[];
        if (regions.isNotEmpty) {
          regionId = regions.first.id;
          return;
        }
        await Future.delayed(const Duration(milliseconds: 50));
      }
    });
    expect(regionId, isNotNull,
        reason:
            'Tapping Start Download must persist a region through OfflineRegionsNotifier.createRegion');

    // Let the orchestrator's Timer.periodic tick in real wall-clock time and
    // drain the queue against the local tile server.
    OfflineRegion? finished;
    await tester.runAsync(() async {
      final deadline = DateTime.now().add(const Duration(seconds: 20));
      while (DateTime.now().isBefore(deadline)) {
        final regions = container.read(offlineRegionsProvider).value ??
            const <OfflineRegion>[];
        final match = regions
            .where((r) => r.id == regionId)
            .cast<OfflineRegion?>()
            .firstOrNull;
        if (match != null &&
            (match.status == DownloadStatus.completed ||
                match.status == DownloadStatus.failed)) {
          finished = match;
          return;
        }
        await Future.delayed(const Duration(milliseconds: 100));
      }
    });

    expect(finished, isNotNull,
        reason: 'orchestrator should reach a terminal state within deadline');
    expect(finished!.status, DownloadStatus.completed,
        reason: 'all tiles served 200, region must complete');
    expect(finished!.totalTiles, greaterThan(0));
    expect(finished!.downloadedTiles, finished!.totalTiles,
        reason: 'every tile must be downloaded');
  }, timeout: const Timeout(Duration(seconds: 45)));
}
