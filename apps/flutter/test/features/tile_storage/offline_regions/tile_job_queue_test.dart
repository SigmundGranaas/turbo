import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:sqflite/sqflite.dart';
import 'package:turbo/core/data/database_provider.dart';
import 'package:turbo/features/tile_storage/offline_regions/data/tile_job_queue.dart';
import 'package:turbo/features/tile_storage/offline_regions/models/tile_download_job.dart';
import 'package:sqflite_common_ffi/sqflite_ffi.dart';

Future<ProviderContainer> createTestContainer() async {
  final db = await databaseFactory.openDatabase(inMemoryDatabasePath);
  await db.execute('''
    CREATE TABLE tile_jobs(
      regionId TEXT NOT NULL,
      providerId TEXT NOT NULL,
      z INTEGER NOT NULL,
      x INTEGER NOT NULL,
      y INTEGER NOT NULL,
      url TEXT NOT NULL,
      status INTEGER NOT NULL,
      attemptCount INTEGER NOT NULL DEFAULT 0,
      workerId TEXT,
      startedAt TEXT,
      PRIMARY KEY (regionId, z, x, y)
    )
  ''');
  await db.execute('CREATE INDEX idx_job_status ON tile_jobs (status)');

  return ProviderContainer(
    overrides: [databaseProvider.overrideWith((ref) async => db)],
  );
}

void main() {
  late ProviderContainer container;
  late TileJobQueue tileJobQueue;
  late Database db;

  const regionId = 'test_region';
  const providerId = 'test_provider';
  const jobs = [
    TileDownloadJob(regionId: regionId, providerId: providerId, z: 1, x: 1, y: 1, url: 'url1'),
    TileDownloadJob(regionId: regionId, providerId: providerId, z: 1, x: 1, y: 2, url: 'url2'),
  ];

  setUpAll(() {
    TestWidgetsFlutterBinding.ensureInitialized();
    sqfliteFfiInit();
    databaseFactory = databaseFactoryFfi;
  });

  setUp(() async {
    container = await createTestContainer();
    // **THE FIX**: Correctly await the future to get the resolved service instance.
    tileJobQueue = await container.read(tileJobQueueProvider.future);
    db = await container.read(databaseProvider.future);
  });

  tearDown(() async {
    await db.close();
    container.dispose();
  });

  group('TileJobQueue', () {
    test('enqueueJobs should insert all jobs as pending', () async {
      await tileJobQueue.enqueueJobs(jobs);

      final count = Sqflite.firstIntValue(await db.rawQuery('SELECT COUNT(*) FROM tile_jobs'));
      expect(count, 2);

      final records = await db.query('tile_jobs');
      expect(records.every((r) => r['status'] == TileJobStatus.pending.index), isTrue);
    });

    test('claimJob should return one pending job and mark it as inProgress', () async {
      await tileJobQueue.enqueueJobs(jobs);

      final claimedJob = await tileJobQueue.claimJob(workerId: 'worker_1');
      expect(claimedJob, isNotNull);

      final dbRecord = await db.query('tile_jobs', where: 'z = ? AND x = ? AND y = ?', whereArgs: [claimedJob!.z, claimedJob.x, claimedJob.y]);
      expect(dbRecord.first['status'], TileJobStatus.inProgress.index);
      expect(dbRecord.first['workerId'], 'worker_1');
      expect(dbRecord.first['attemptCount'], 1);

      final remainingCount = Sqflite.firstIntValue(await db.rawQuery('SELECT COUNT(*) FROM tile_jobs WHERE status = ?', [TileJobStatus.pending.index]));
      expect(remainingCount, 1);
    });

    test('claimJob should return null if no pending jobs are available', () async {
      expect(await tileJobQueue.claimJob(workerId: 'worker_1'), isNull);
    });

    test('markJobSuccess should remove the job from the queue', () async {
      await tileJobQueue.enqueueJobs(jobs);
      final jobToSucceed = jobs.first;
      await tileJobQueue.markJobSuccess(jobToSucceed);
      final count = Sqflite.firstIntValue(await db.rawQuery('SELECT COUNT(*) FROM tile_jobs'));
      expect(count, 1);
    });

    test('markJobFailed with attempts remaining should reset job to pending', () async {
      await tileJobQueue.enqueueJobs(jobs);
      final jobToFail = (await tileJobQueue.claimJob(workerId: 'worker_1'))!;
      final jobWithUpdatedAttempts = jobToFail.copyWith(attemptCount: 1);
      await tileJobQueue.markJobFailed(jobWithUpdatedAttempts);
      final dbRecord = await db.query('tile_jobs', where: 'z = ?', whereArgs: [jobToFail.z]);
      expect(dbRecord.first['status'], TileJobStatus.pending.index);
      expect(dbRecord.first['workerId'], isNull);
      expect(dbRecord.first['attemptCount'], 1);
    });

    test('markJobFailed on final attempt should remove the job', () async {
      await tileJobQueue.enqueueJobs(jobs);
      final jobToFail = (await tileJobQueue.claimJob(workerId: 'worker_1'))!.copyWith(attemptCount: 3);
      await tileJobQueue.markJobFailed(jobToFail);
      final remainingCount = Sqflite.firstIntValue(await db.rawQuery('SELECT COUNT(*) FROM tile_jobs'));
      expect(remainingCount, 1);
    });
  });
}