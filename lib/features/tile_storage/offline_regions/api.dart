library offline_regions;

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
import '../tile_store/api.dart';
import '../tile_store/utils/tile_provider_id_sanitizer.dart';
import 'data/offline_tile_provider.dart';

// --- Exports ---

// Models
export 'models/offline_region.dart';

// UI
export 'widgets/offline_regions_page.dart';
export 'widgets/region_creation_page.dart';

// --- Providers ---

final offlineRegionsProvider =
AsyncNotifierProvider<OfflineRegionsNotifier, List<OfflineRegion>>(
      () {
    // This feature is not available on the web
    if (kIsWeb) return OfflineRegionsNotifier.web();
    return OfflineRegionsNotifier();
  },
);

class OfflineRegionsNotifier extends AsyncNotifier<List<OfflineRegion>> {
  bool _isWeb = false;

  // Private constructor for the web version
  OfflineRegionsNotifier.web() {
    _isWeb = true;
  }
  // Public constructor for native
  OfflineRegionsNotifier();

  @override
  Future<List<OfflineRegion>> build() async {
    if (_isWeb) return []; // Return empty list for web
    // **THE FIX**: Break the circular dependency.
    // The notifier no longer watches the orchestrator.
    // It relies on the orchestrator being started elsewhere (e.g., in main or a test setup).
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
    required List<TileCoordinates> coords,
  }) async {
    if (_isWeb) return;

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
    final currentState = state.valueOrNull ?? [];
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

  Future<void> deleteRegion(String regionId) async {
    if (_isWeb) return;

    final repo = await ref.read(regionRepositoryProvider.future);
    final jobQueue = await ref.read(tileJobQueueProvider.future);
    final tileStore = await ref.read(tileStoreServiceProvider.future);

    final region = await repo.getRegion(regionId);
    if (region == null) return;

    final providerId = sanitizeProviderId(region.urlTemplate);
    final coords =
    _calculateCoordsForRegion(region.bounds, region.minZoom, region.maxZoom);

    // FIX: Using the now public db instance from TileStoreService for a batch
    final batch = (tileStore.db).batch();
    for (final coord in coords) {
      batch.rawUpdate(
          'UPDATE tile_store SET referenceCount = referenceCount - 1 WHERE providerId = ? AND z = ? AND x = ? AND y = ? AND referenceCount > 0',
          [providerId, coord.z, coord.x, coord.y]);
    }
    await batch.commit(noResult: true);

    await jobQueue.clearJobsForRegion(regionId);
    await repo.deleteRegion(regionId);

    final currentState = state.valueOrNull ?? [];
    state = AsyncData(currentState.where((r) => r.id != regionId).toList());
  }

  Future<void> updateRegionProgress(String regionId,
      {required bool tileSucceeded}) async {
    if (_isWeb) return;
    final repo = await ref.read(regionRepositoryProvider.future);

    if (tileSucceeded) {
      await repo.incrementDownloadedTileCount(regionId);
    }

    final currentRegions = state.valueOrNull;
    if (currentRegions == null) return;

    final regionIndex = currentRegions.indexWhere((r) => r.id == regionId);
    if (regionIndex == -1) return;

    final oldRegion = currentRegions[regionIndex];
    if (oldRegion.status != DownloadStatus.downloading) return;

    final updatedRegion = oldRegion.copyWith(
      downloadedTiles:
      tileSucceeded ? oldRegion.downloadedTiles + 1 : oldRegion.downloadedTiles,
    );

    final newList = List<OfflineRegion>.from(currentRegions);
    newList[regionIndex] = updatedRegion;
    state = AsyncData(newList);
  }

  Future<void> finalizeRegion(String regionId) async {
    if (_isWeb) return;
    final repo = await ref.read(regionRepositoryProvider.future);

    final currentRegions = state.valueOrNull;
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
}

final offlineApiProvider = Provider<OfflineApi>((ref) => OfflineApi(ref));

class OfflineApi {
  final Ref _ref;
  OfflineApi(this._ref);

  Future<void> downloadRegion({
    required String name,
    required LatLngBounds bounds,
    required int minZoom,
    required int maxZoom,
    required String urlTemplate,
    required String tileProviderId,
    required String tileProviderName,
  }) async {
    if (kIsWeb) return Future.value();

    final coords = _calculateCoordsForRegion(bounds, minZoom, maxZoom);
    if (coords.isEmpty) return Future.value();

    await _ref.read(offlineRegionsProvider.notifier).createRegion(
      name: name,
      bounds: bounds,
      minZoom: minZoom,
      maxZoom: maxZoom,
      urlTemplate: urlTemplate,
      tileProviderId: tileProviderId,
      tileProviderName: tileProviderName,
      coords: coords,
    );
  }

  TileProvider? createTileProvider({required OfflineRegion region}) {
    if (kIsWeb) return null;
    final tileStoreAsync = _ref.watch(tileStoreServiceProvider);
    return tileStoreAsync.when(
      data: (store) => OfflineTileProvider(region: region, tileStore: store),
      loading: () => null,
      error: (_, __) => null,
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
    final nwPoint = crs.latLngToPoint(bounds.northWest, zoom);
    final sePoint = crs.latLngToPoint(bounds.southEast, zoom);
    final nwTile = Point<int>(
        (nwPoint.x / tileSize).floor(), (nwPoint.y / tileSize).floor());
    final seTile = Point<int>(
        (sePoint.x / tileSize).floor(), (sePoint.y / tileSize).floor());

    for (var x = nwTile.x; x <= seTile.x; x++) {
      for (var y = nwTile.y; y <= seTile.y; y++) {
        coordsList.add(TileCoordinates(x, y, z));
      }
    }
  }
  return coordsList;
}