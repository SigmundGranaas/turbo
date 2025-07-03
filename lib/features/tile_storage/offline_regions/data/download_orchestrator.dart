import 'dart:async';
import 'dart:isolate';
import 'package:flutter_map/flutter_map.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:turbo/features/tile_storage/offline_regions/data/region_repository.dart';
import 'package:turbo/features/tile_storage/offline_regions/data/tile_job_queue.dart';
import 'package:turbo/core/service/logger.dart';
import 'package:uuid/uuid.dart';
import '../../tile_store/api.dart';
import '../api.dart';
import '../models/tile_download_job.dart';
import 'download_worker.dart';

final downloadOrchestratorProvider = Provider<DownloadOrchestrator?>((ref) {
  // Watch all async dependencies.
  final jobQueueAsync = ref.watch(tileJobQueueProvider);
  final tileStoreAsync = ref.watch(tileStoreServiceProvider);
  final regionRepoAsync = ref.watch(regionRepositoryProvider);

  // If any of them are not ready, return null. The UI/caller won't get an instance yet.
  if (jobQueueAsync.isLoading || tileStoreAsync.isLoading || regionRepoAsync.isLoading) {
    return null;
  }

  // Optionally handle errors, e.g., by logging them.
  if (jobQueueAsync.hasError || tileStoreAsync.hasError || regionRepoAsync.hasError) {
    log.severe('Failed to initialize a dependency for DownloadOrchestrator',
        jobQueueAsync.error ?? tileStoreAsync.error ?? regionRepoAsync.error);
    return null;
  }

  // All dependencies are ready, create the real orchestrator instance.
  // Riverpod will cache this instance. It will only be recreated if one of the
  // dependencies is invalidated and re-resolved.
  final orchestrator = DownloadOrchestrator(
    jobQueue: jobQueueAsync.value!,
    tileStore: tileStoreAsync.value!,
    regionRepository: regionRepoAsync.value!,
    regionNotifier: ref.read(offlineRegionsProvider.notifier),
  );

  orchestrator.start();
  ref.onDispose(() => orchestrator.stop());

  return orchestrator;
});

typedef IsolateSpawner = Future<Isolate> Function(
    void Function(DownloadTask) entryPoint, DownloadTask message);

class DownloadOrchestrator {
  final TileJobQueue _jobQueue;
  final TileStoreService _tileStore;
  final RegionRepository _regionRepository;
  final OfflineRegionsNotifier _regionNotifier;
  final IsolateSpawner _isolateSpawner;

  Timer? _timer;
  bool _isRunning = false;
  bool _isTicking = false; // Mutex flag to prevent concurrent ticks.
  static const int maxConcurrentWorkers = 4;
  final Map<String, Isolate> _activeWorkers = {};

  DownloadOrchestrator({
    required TileJobQueue jobQueue,
    required TileStoreService tileStore,
    required RegionRepository regionRepository,
    required OfflineRegionsNotifier regionNotifier,
    IsolateSpawner spawner = Isolate.spawn,
  })  : _jobQueue = jobQueue,
        _tileStore = tileStore,
        _regionRepository = regionRepository,
        _regionNotifier = regionNotifier,
        _isolateSpawner = spawner;

  void start() {
    if (_isRunning) return;
    _isRunning = true;
    log.info('DownloadOrchestrator started.');
    _timer = Timer.periodic(const Duration(seconds: 2), (_) => _tick());
    _tick();
  }

  void stop() {
    log.info('DownloadOrchestrator stopped.');
    _timer?.cancel();
    _isRunning = false;
    _activeWorkers
        .forEach((id, isolate) => isolate.kill(priority: Isolate.immediate));
    _activeWorkers.clear();
  }

  Future<void> _tick() async {
    if (_isTicking || !_isRunning) return;
    _isTicking = true;

    try {
      final staleCount = await _jobQueue.findAndResetStaleJobs();
      if (staleCount > 0) {
        log.warning('Reset $staleCount stale jobs.');
      }

      final availableSlots = maxConcurrentWorkers - _activeWorkers.length;
      if (availableSlots <= 0) return;

      for (int i = 0; i < availableSlots; i++) {
        final workerId = const Uuid().v4();
        final job = await _jobQueue.claimJob(workerId: workerId);
        if (job != null) {
          _spawnWorker(job, workerId);
        } else {
          break; // No more pending jobs
        }
      }
    } finally {
      _isTicking = false;
    }
  }

  void _spawnWorker(TileDownloadJob job, String workerId) {
    log.fine('Spawning worker $workerId for job ${job.url}');
    final receivePort = ReceivePort();

    receivePort.listen((message) {
      if (message is JobResult) {
        _handleJobResult(message).whenComplete(() {
          _activeWorkers.remove(workerId)?.kill();
          receivePort.close();
          _tick(); // Check for more work immediately
        });
      } else if (message is WorkerLogRecord) {
        log.log(message.level, '[Worker $workerId] ${message.message}',
            message.error, message.stackTrace);
      } else if (message is List && message.length == 2 && message[0] is String) {
        final error = message[0];
        final stack = message[1];
        log.severe('[Worker $workerId] Unhandled isolate error', error,
            StackTrace.fromString(stack));
        _handleJobResult(JobFailure(job, "Isolate crashed: $error"))
            .whenComplete(() {
          _activeWorkers.remove(workerId);
          receivePort.close();
          _tick();
        });
      } else {
        log.warning(
            'Orchestrator received unknown message from worker: $message');
      }
    });

    _isolateSpawner(
        downloadWorkerEntrypoint, DownloadTask(receivePort.sendPort, job))
        .then((isolate) {
      log.fine('Worker $workerId for job ${job.url} spawned successfully.');
      _activeWorkers[workerId] = isolate;
      isolate.addErrorListener(receivePort.sendPort);
    }).catchError((error, stack) {
      log.severe('Isolate.spawn FAILED for job ${job.url}', error, stack);
      receivePort.close();
      _activeWorkers.remove(workerId);
      _handleJobResult(JobFailure(job, 'Failed to spawn isolate: $error'));
    });
  }

  Future<void> _handleJobResult(JobResult result) async {
    try {
      final job = result.job;
      final coords = TileCoordinates(job.x, job.y, job.z);

      if (result is JobSuccess) {
        log.fine(
            'Job SUCCEEDED for ${job.url} in ${result.duration.inMilliseconds}ms');
        await _tileStore.put(job.providerId, coords, result.bytes);
        await _tileStore.incrementReference(job.providerId, coords);
        await _jobQueue.markJobSuccess(job);
        await _regionNotifier.updateRegionProgress(job.regionId,
            tileSucceeded: true);
      } else if (result is JobFailure) {
        log.warning('Job FAILED for ${job.url}. Error: ${result.error}');
        await _jobQueue.markJobFailed(job);
        await _regionNotifier.updateRegionProgress(job.regionId,
            tileSucceeded: false);
      }

      await _checkAndFinalizeRegion(job.regionId);
    } catch (e, s) {
      log.severe(
        'CRITICAL: Error while processing job result for region ${result.job.regionId}. '
            'This may lead to an incomplete or stalled download. Error: $e',
        e,
        s,
      );
    }
  }

  Future<void> _checkAndFinalizeRegion(String regionId) async {
    final region = await _regionRepository.getRegion(regionId);
    if (region != null && region.status == DownloadStatus.downloading) {
      final remaining = await _jobQueue.getRemainingJobCount(region.id);
      if (remaining == 0) {
        log.info('FINALIZING region ${region.id}.');
        await _regionNotifier.finalizeRegion(region.id);
      }
    }
  }
}