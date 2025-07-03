import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:sqflite/sqflite.dart';
import 'package:turbo/features/tile_storage/offline_regions/models/tile_download_job.dart';

import '../../../../core/data/database_provider.dart';

final tileJobQueueProvider = FutureProvider<TileJobQueue>((ref) async {
  // This will throw if run on web, which is correct as this feature is not for web.
  final db = await ref.watch(databaseProvider.future);
  return TileJobQueue(db);
});

/// Manages the persistent queue of tiles to be downloaded for offline regions.
class TileJobQueue {
  final Database db;
  static const int _maxJobAttempts = 3;

  TileJobQueue(this.db);

  Future<void> enqueueJobs(List<TileDownloadJob> jobs) async {
    final batch = db.batch();
    for (final job in jobs) {
      batch.insert(
        tileJobsTable,
        job.toNewJobMap(),
        conflictAlgorithm: ConflictAlgorithm.ignore,
      );
    }
    await batch.commit(noResult: true);
  }

  Future<TileDownloadJob?> claimJob({required String workerId}) async {
    TileDownloadJob? claimedJob;

    await db.transaction((txn) async {
      final pendingJobs = await txn.query(
        tileJobsTable,
        where: 'status = ? AND attemptCount < ?',
        whereArgs: [TileJobStatus.pending.index, _maxJobAttempts],
        orderBy: 'attemptCount ASC, z ASC',
        limit: 1,
      );

      if (pendingJobs.isNotEmpty) {
        final jobMap = pendingJobs.first;
        final job = TileDownloadJob.fromMap(jobMap);

        final now = DateTime.now();
        final updatedCount = await txn.update(
          tileJobsTable,
          {
            'status': TileJobStatus.inProgress.index,
            'workerId': workerId,
            'startedAt': now.toIso8601String(),
            'attemptCount': job.attemptCount + 1,
          },
          where: 'regionId = ? AND z = ? AND x = ? AND y = ? AND status = ?',
          whereArgs: [
            job.regionId,
            job.z,
            job.x,
            job.y,
            TileJobStatus.pending.index
          ],
        );

        if (updatedCount > 0) {
          claimedJob = job.copyWith(
            status: TileJobStatus.inProgress,
            attemptCount: job.attemptCount + 1,
            workerId: workerId,
            startedAt: now,
          );
        }
      }
    });
    return claimedJob;
  }

  Future<void> markJobSuccess(TileDownloadJob job) async {
    await db.delete(
      tileJobsTable,
      where: 'regionId = ? AND z = ? AND x = ? AND y = ?',
      whereArgs: [job.regionId, job.z, job.x, job.y],
    );
  }

  Future<void> markJobFailed(TileDownloadJob job) async {
    if (job.attemptCount >= _maxJobAttempts) {
      await db.delete(
        tileJobsTable,
        where: 'regionId = ? AND z = ? AND x = ? AND y = ?',
        whereArgs: [job.regionId, job.z, job.x, job.y],
      );
    } else {
      await db.update(
        tileJobsTable,
        {'status': TileJobStatus.pending.index, 'workerId': null},
        where: 'regionId = ? AND z = ? AND x = ? AND y = ?',
        whereArgs: [job.regionId, job.z, job.x, job.y],
      );
    }
  }

  Future<int> findAndResetStaleJobs(
      {Duration staleAfter = const Duration(minutes: 1)}) async {
    final staleTimestamp =
    DateTime.now().subtract(staleAfter).toIso8601String();

    return await db.update(
      tileJobsTable,
      {'status': TileJobStatus.pending.index, 'workerId': null},
      where: 'status = ? AND startedAt < ?',
      whereArgs: [TileJobStatus.inProgress.index, staleTimestamp],
    );
  }

  Future<int> getRemainingJobCount(String regionId) async {
    return Sqflite.firstIntValue(await db.rawQuery(
        'SELECT COUNT(*) FROM $tileJobsTable WHERE regionId = ?', [regionId])) ??
        0;
  }

  Future<void> clearJobsForRegion(String regionId) async {
    await db.delete(tileJobsTable, where: 'regionId = ?', whereArgs: [regionId]);
  }
}