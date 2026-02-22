import 'dart:async';
import 'package:dio/dio.dart';
import 'package:flutter/foundation.dart';
import 'package:flutter_map/flutter_map.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:turbo/features/tile_storage/offline_regions/data/region_repository.dart';
import 'package:turbo/features/tile_storage/offline_regions/data/tile_job_queue.dart';
import 'package:turbo/core/service/logger.dart';
import 'package:uuid/uuid.dart';
import '../../tile_store/api.dart';
import '../api.dart';
import '../models/tile_download_job.dart';

final downloadOrchestratorProvider = Provider<DownloadOrchestrator?>((ref) {
  // Watch all async dependencies.
  final jobQueueAsync = ref.watch(tileJobQueueProvider);
  final tileStoreAsync = ref.watch(tileStoreServiceProvider);
  final regionRepoAsync = ref.watch(regionRepositoryProvider);

  // If any of them are not ready, return null.
  if (jobQueueAsync.isLoading || tileStoreAsync.isLoading || regionRepoAsync.isLoading) {
    return null;
  }

  if (jobQueueAsync.hasError || tileStoreAsync.hasError || regionRepoAsync.hasError) {
    log.severe('Failed to initialize a dependency for DownloadOrchestrator',
        jobQueueAsync.error ?? tileStoreAsync.error ?? regionRepoAsync.error);
    return null;
  }

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

class DownloadOrchestrator {
  final TileJobQueue _jobQueue;
  final TileStoreService _tileStore;
  final RegionRepository _regionRepository;
  final OfflineRegionsNotifier _regionNotifier;
  
  final Dio _dio;
  Timer? _timer;
  bool _isRunning = false;
  bool _isTicking = false;
  
  static const int maxConcurrentDownloads = 8;
  int _activeDownloadCount = 0;

  // Circuit breaker state
  int _consecutiveConnectionFailures = 0;
  DateTime? _pauseUntil;

  DownloadOrchestrator({
    required TileJobQueue jobQueue,
    required TileStoreService tileStore,
    required RegionRepository regionRepository,
    required OfflineRegionsNotifier regionNotifier,
  })  : _jobQueue = jobQueue,
        _tileStore = tileStore,
        _regionRepository = regionRepository,
        _regionNotifier = regionNotifier,
        _dio = Dio(BaseOptions(
          responseType: ResponseType.bytes,
          connectTimeout: const Duration(seconds: 10),
          receiveTimeout: const Duration(seconds: 10),
          headers: {
            'User-Agent': 'turbo_map_app/1.0.18 (+https://github.com/sigmundgranaas/turbo)',
            'Accept': 'image/png,image/*;q=0.8,*/*;q=0.5',
          },
        ));

  void start() {
    if (_isRunning) return;
    _isRunning = true;
    log.info('DownloadOrchestrator started.');
    _timer = Timer.periodic(const Duration(seconds: 1), (_) => _tick());
    _tick();
  }

  void stop() {
    log.info('DownloadOrchestrator stopped.');
    _timer?.cancel();
    _isRunning = false;
  }

  Future<void> _tick() async {
    if (_isTicking || !_isRunning) return;

    // Check if we are currently paused by the circuit breaker
    if (_pauseUntil != null) {
      if (DateTime.now().isBefore(_pauseUntil!)) {
        return;
      }
      log.info('Circuit breaker: Resuming downloads after connection pause.');
      _pauseUntil = null;
      _consecutiveConnectionFailures = 0;
    }

    _isTicking = true;

    try {
      // 1. Recover stale jobs (e.g. from app crash)
      await _jobQueue.findAndResetStaleJobs();

      // 2. Fill available slots
      final availableSlots = maxConcurrentDownloads - _activeDownloadCount;
      if (availableSlots <= 0) return;

      for (int i = 0; i < availableSlots; i++) {
        final workerId = "async-worker-${const Uuid().v4()}";
        final job = await _jobQueue.claimJob(workerId: workerId);
        if (job != null) {
          unawaited(_processJob(job));
        } else {
          break; // No more work
        }
      }
    } finally {
      _isTicking = false;
    }
  }

  Future<void> _processJob(TileDownloadJob job) async {
    _activeDownloadCount++;
    final coords = TileCoordinates(job.x, job.y, job.z);
    
    try {
      final response = await _dio.get<Uint8List>(job.url);
      
      if (response.statusCode == 200 && response.data != null) {
        // Success: Reset connection failure counter
        _consecutiveConnectionFailures = 0;
        
        await _tileStore.putWithReference(job.providerId, coords, response.data!);
        await _jobQueue.markJobSuccess(job);
        await _regionNotifier.updateRegionProgress(job.regionId, tileSucceeded: true);
      } else {
        throw Exception("Status code: ${response.statusCode}");
      }
    } catch (e) {
      log.warning('Job FAILED for ${job.url}. Error: $e');
      
      // Check if this is a connection-level error
      if (e is DioException) {
        final isConnectionError = e.type == DioExceptionType.connectionError || 
                                 e.type == DioExceptionType.connectionTimeout ||
                                 e.error.toString().contains('SocketException') ||
                                 e.error.toString().contains('Failed host lookup');
        
        if (isConnectionError) {
          _consecutiveConnectionFailures++;
          if (_consecutiveConnectionFailures >= 5) {
            log.severe('Circuit breaker: Persistent connection errors detected. Pausing downloads for 30 seconds.');
            _pauseUntil = DateTime.now().add(const Duration(seconds: 30));
          }
        }
      }

      await _jobQueue.markJobFailed(job);
      await _regionNotifier.updateRegionProgress(job.regionId, tileSucceeded: false);
    } finally {
      _activeDownloadCount--;
      await _checkAndFinalizeRegion(job.regionId);
      // Immediately try to start next job if not paused
      if (_pauseUntil == null) {
        _tick();
      }
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