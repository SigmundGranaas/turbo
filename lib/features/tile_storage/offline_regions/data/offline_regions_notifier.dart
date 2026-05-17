import 'dart:async';
import 'dart:math';

import 'package:flutter/foundation.dart';
import 'package:flutter_map/flutter_map.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:turbo/features/tile_storage/offline_regions/data/region_repository.dart';
import 'package:turbo/features/tile_storage/offline_regions/data/tile_job_queue.dart';
import 'package:turbo/features/tile_storage/offline_regions/models/offline_region.dart';
import 'package:turbo/features/tile_storage/offline_regions/models/tile_download_job.dart';
import 'package:uuid/uuid.dart';
import '../../tile_store/api.dart';
import 'offline_tile_provider.dart';

final offlineRegionsProvider =
    AsyncNotifierProvider<OfflineRegionsNotifier, List<OfflineRegion>>(
  OfflineRegionsNotifier.new,
);

class OfflineRegionsNotifier extends AsyncNotifier<List<OfflineRegion>> {
  DateTime _lastUiUpdateTime = DateTime.fromMillisecondsSinceEpoch(0);
  final Map<String, int> _progressBuffer = {};

  @override
  Future<List<OfflineRegion>> build() async {
    if (kIsWeb) return [];

    // The notifier no longer watches the orchestrator to avoid a circular
    // dependency. The orchestrator must be started elsewhere.
    final repo = await ref.watch(regionRepositoryProvider.future);
    return repo.getAllRegions();
  }

  Future<void> createRegion({
    required String name,
    required LatLngBounds bounds,
    required int minZoom,
    required int maxZoom,
    required String urlTemplate,
    required String tileProviderId,
    required String tileProviderName,
  }) async {
    if (kIsWeb) return;

    final coords = _calculateCoordsForRegion(bounds, minZoom, maxZoom);
    if (coords.isEmpty) return;

    final repo = await ref.read(regionRepositoryProvider.future);
    final jobQueue = await ref.read(tileJobQueueProvider.future);
    final providerId = sanitizeProviderId(urlTemplate);
    final regionId = const Uuid().v4();

    final newRegion = OfflineRegion(
      id: regionId,
      name: name,
      bounds: bounds,
      minZoom: minZoom,
      maxZoom: maxZoom,
      urlTemplate: urlTemplate,
      tileProviderId: tileProviderId,
      tileProviderName: tileProviderName,
      status: DownloadStatus.downloading,
      totalTiles: coords.length,
    );

    await repo.saveRegion(newRegion);
    final currentState = state.value ?? [];
    state = AsyncData([...currentState, newRegion]);

    final jobs = coords.map((c) {
      final url = getTileUrl(urlTemplate, c);
      return TileDownloadJob(
        regionId: newRegion.id,
        providerId: providerId,
        z: c.z,
        x: c.x,
        y: c.y,
        url: url,
      );
    }).toList();
    await jobQueue.enqueueJobs(jobs);
  }

  /// Removes every region older than [cutoff]. Each deletion goes through
  /// [deleteRegion] so tile reference counts and the in-flight job queue are
  /// kept consistent. Returns the number of regions deleted.
  Future<int> deleteOlderThan(DateTime cutoff) async {
    if (kIsWeb) return 0;
    final current = state.value ?? [];
    final stale = current.where((r) => r.createdAt.isBefore(cutoff)).toList();
    for (final r in stale) {
      await deleteRegion(r.id);
    }
    return stale.length;
  }

  Future<void> deleteRegion(String regionId) async {
    if (kIsWeb) return;

    final repo = await ref.read(regionRepositoryProvider.future);
    final jobQueue = await ref.read(tileJobQueueProvider.future);
    final tileStore = await ref.read(tileStoreServiceProvider.future);

    final region = await repo.getRegion(regionId);
    if (region == null) return;

    final providerId = sanitizeProviderId(region.urlTemplate);
    final coords =
        _calculateCoordsForRegion(region.bounds, region.minZoom, region.maxZoom);

    final batch = (tileStore.db).batch();
    for (final coord in coords) {
      batch.rawUpdate(
          'UPDATE tile_store SET referenceCount = referenceCount - 1 WHERE providerId = ? AND z = ? AND x = ? AND y = ? AND referenceCount > 0',
          [providerId, coord.z, coord.x, coord.y]);
    }
    await batch.commit(noResult: true);

    await jobQueue.clearJobsForRegion(regionId);
    await repo.deleteRegion(regionId);

    final currentState = state.value ?? [];
    state = AsyncData(currentState.where((r) => r.id != regionId).toList());
  }

  Future<void> updateRegionProgress(String regionId,
      {required bool tileSucceeded}) async {
    if (kIsWeb) return;

    _progressBuffer[regionId] =
        (_progressBuffer[regionId] ?? 0) + (tileSucceeded ? 1 : 0);

    final now = DateTime.now();
    if (now.difference(_lastUiUpdateTime).inSeconds >= 1 ||
        (_progressBuffer[regionId] ?? 0) >= 20) {
      final increment = _progressBuffer.remove(regionId) ?? 0;
      _lastUiUpdateTime = now;

      if (increment > 0) {
        final repo = await ref.read(regionRepositoryProvider.future);
        await repo.incrementDownloadedTileCount(regionId, count: increment);
      }

      final currentRegions = state.value;
      if (currentRegions == null) return;

      final regionIndex = currentRegions.indexWhere((r) => r.id == regionId);
      if (regionIndex == -1) return;

      final oldRegion = currentRegions[regionIndex];
      if (oldRegion.status != DownloadStatus.downloading) return;

      final updatedRegion = oldRegion.copyWith(
        downloadedTiles: oldRegion.downloadedTiles + increment,
      );

      final newList = List<OfflineRegion>.from(currentRegions);
      newList[regionIndex] = updatedRegion;
      state = AsyncData(newList);
    }
  }

  Future<void> finalizeRegion(String regionId) async {
    if (kIsWeb) return;

    final remainingIncrement = _progressBuffer.remove(regionId) ?? 0;
    if (remainingIncrement > 0) {
      final repo = await ref.read(regionRepositoryProvider.future);
      await repo.incrementDownloadedTileCount(regionId,
          count: remainingIncrement);
    }

    final repo = await ref.read(regionRepositoryProvider.future);

    final currentRegions = state.value;
    if (currentRegions == null) return;

    final regionIndex = currentRegions.indexWhere((r) => r.id == regionId);
    if (regionIndex == -1) return;

    final finalRegionFromRepo = await repo.getRegion(regionId);
    if (finalRegionFromRepo == null) return;

    final finalRegion =
        finalRegionFromRepo.copyWith(status: DownloadStatus.completed);

    await repo.saveRegion(finalRegion);

    final newList = List<OfflineRegion>.from(currentRegions);
    newList[regionIndex] = finalRegion;
    state = AsyncData(newList);
  }

  TileProvider? createTileProvider({required OfflineRegion region}) {
    if (kIsWeb) return null;
    final tileStoreAsync = ref.read(tileStoreServiceProvider);
    return tileStoreAsync.whenOrNull(
      data: (store) => OfflineTileProvider(region: region, tileStore: store),
    );
  }
}

String getTileUrl(String urlTemplate, TileCoordinates c) {
  return urlTemplate
      .replaceAll('{z}', c.z.toString())
      .replaceAll('{x}', c.x.toString())
      .replaceAll('{y}', c.y.toString())
      .replaceAll('{s}', ['a', 'b', 'c'][(c.x + c.y) % 3]);
}

List<TileCoordinates> _calculateCoordsForRegion(
    LatLngBounds bounds, int minZoom, int maxZoom) {
  final coordsList = <TileCoordinates>[];
  const crs = Epsg3857();
  const tileSize = 256.0;

  for (var z = minZoom; z <= maxZoom; z++) {
    final zoom = z.toDouble();
    final nwPoint = crs.latLngToOffset(bounds.northWest, zoom);
    final sePoint = crs.latLngToOffset(bounds.southEast, zoom);

    // Small epsilon avoids including the next tile when the coordinate
    // sits exactly on a tile boundary.
    final nwTileX = (nwPoint.dx / tileSize).floor();
    final nwTileY = (nwPoint.dy / tileSize).floor();
    final seTileX = ((sePoint.dx - 0.0000001) / tileSize).floor();
    final seTileY = ((sePoint.dy - 0.0000001) / tileSize).floor();

    final maxTile = pow(2, z).toInt() - 1;

    for (var x = max(0, nwTileX); x <= min(maxTile, seTileX); x++) {
      for (var y = max(0, nwTileY); y <= min(maxTile, seTileY); y++) {
        coordsList.add(TileCoordinates(x, y, z));
      }
    }
  }
  return coordsList;
}
