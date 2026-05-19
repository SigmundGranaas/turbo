import 'package:flutter/foundation.dart';
import 'package:flutter_map/flutter_map.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'custom_provider_store.dart';
import 'layer_preference_service.dart';
import 'package:turbo/features/tile_providers/data/providers/avalanche_overlay.dart';
import 'package:turbo/features/tile_providers/data/providers/google_sattelite.dart';
import 'package:turbo/features/tile_providers/data/providers/nasjonal_turbase_overlay.dart';
import 'package:turbo/features/tile_providers/data/providers/norges_kart_topo.dart';
import 'package:turbo/features/tile_providers/data/providers/offline_region_provider_config.dart';
import 'package:turbo/features/tile_providers/data/providers/openseamap_overlay.dart';
import 'package:turbo/features/tile_providers/data/providers/osm_tiles.dart';
import 'package:turbo/features/tile_providers/data/providers/sjokart_raster.dart';
import 'package:turbo/features/tile_providers/data/providers/vector_path_overlays.dart';
import 'package:turbo/features/tile_providers/models/custom_tile_provider.dart';
import 'package:turbo/features/tile_providers/models/tile_provider_config.dart';
import 'package:turbo/features/tile_providers/models/tile_registry_state.dart';
import 'package:turbo/features/tile_storage/cached_tiles/api.dart';
import 'package:turbo/features/tile_storage/offline_regions/api.dart';
import 'package:turbo/features/tile_storage/tile_store/api.dart';

final tileRegistryProvider =
    NotifierProvider<TileRegistry, TileRegistryState>(TileRegistry.new);

class TileRegistry extends Notifier<TileRegistryState> {
  @override
  TileRegistryState build() {
    // --- 1. Register built-in providers ---
    final builtInProviders = [
      NorgeskartTopoConfig(),
      SjokartRasterConfig(),
      OsmConfig(),
      GoogleSatelliteConfig(),
      AvalancheOverlayConfig(),
      OpenSeaMapOverlayConfig(),
      TrailsFootOverlayConfig(),
      TrailsSkiOverlayConfig(),
      TrailsBikeOverlayConfig(),
      TrailsOtherOverlayConfig(),
      OsmPathsOverlayConfig(),
      N50StiOverlayConfig(),
    ];
    final initialProviders = <String, TileProviderConfig>{
      for (var p in builtInProviders) p.id: p,
    };

    // Merge any already-loaded custom providers without re-subscribing
    // (ref.watch would force build() to re-run on every state transition of
    // the customs notifier — including its loading -> data flip — wiping
    // accumulated state). ref.listen merges future changes in place.
    final customsAsync = ref.read(customProviderStoreProvider);
    for (final c in customsAsync.value ?? const <CustomTileProvider>[]) {
      initialProviders[c.id] = CustomTileProviderConfig(c);
    }
    ref.listen(customProviderStoreProvider, (prev, next) {
      _syncCustomProviders(next.value ?? const <CustomTileProvider>[]);
    });

    // --- 2. Load preferences ---
    final preferencesFuture =
    ref.read(layerPreferenceServiceProvider).getSavedLayers();

    preferencesFuture.then((savedLayers) {
      final hasSavedLocal = savedLayers['local']?.isNotEmpty ?? false;
      final hasSavedGlobal = savedLayers['global']?.isNotEmpty ?? false;
      final savedOffline = savedLayers['offline'] ?? [];

      if (hasSavedLocal || hasSavedGlobal) {
        state = state.copyWith(
          activeGlobalIds: savedLayers['global']!,
          activeLocalIds: savedLayers['local']!,
          activeOverlayIds: savedLayers['overlays']!,
          activeOfflineIds: savedOffline,
        );
      } else {
        // Apply default on first launch
        toggleLocalLayer('topo');
        // Offline layers are enabled by default on download
      }
    });

    // --- 3. Listen for changes in offline regions ---
    if (!kIsWeb) {
      ref.listen(offlineRegionsProvider, (previous, next) {
        if (!next.hasValue) return;
        _syncOfflineProviders(next.value!);
      });
    }

    // --- 4. Return initial empty state, will be populated by async ops ---
    return TileRegistryState(
      availableProviders: initialProviders,
      activeGlobalIds: const [],
      activeLocalIds: const [],
      activeOverlayIds: const [],
      activeOfflineIds: const [],
    );
  }

  void _syncCustomProviders(List<CustomTileProvider> customs) {
    final next = Map<String, TileProviderConfig>.from(state.availableProviders);
    // Drop any existing custom entries (id has the 'custom_' prefix) and
    // re-add the current set. Built-ins and offline regions are untouched.
    next.removeWhere((id, _) => id.startsWith('custom_'));
    for (final c in customs) {
      next[c.id] = CustomTileProviderConfig(c);
    }
    // Drop active selections that point to a deleted custom id so the
    // registry doesn't carry stale references.
    final customIds = customs.map((c) => c.id).toSet();
    bool keep(String id) => !id.startsWith('custom_') || customIds.contains(id);
    state = state.copyWith(
      availableProviders: next,
      activeGlobalIds: state.activeGlobalIds.where(keep).toList(),
      activeLocalIds: state.activeLocalIds.where(keep).toList(),
      activeOverlayIds: state.activeOverlayIds.where(keep).toList(),
    );
  }

  void _syncOfflineProviders(List<OfflineRegion> regions) {
    final Map<String, TileProviderConfig> currentProviders =
    Map.from(state.availableProviders);
    final Set<String> currentKnownOfflineIds = currentProviders.values
        .where((p) => p.category == TileProviderCategory.offline)
        .map((p) => p.id)
        .toSet();

    final Set<String> newRegionIds = regions.map((r) => r.id).toSet();
    final Set<String> newlyAddedIds = newRegionIds.difference(currentKnownOfflineIds);

    // Remove providers for deleted regions
    final toRemove = currentKnownOfflineIds.difference(newRegionIds);
    for (final id in toRemove) {
      currentProviders.remove(id);
    }

    // Add or update providers for new/changed regions
    for (final region in regions) {
      currentProviders[region.id] = OfflineRegionProviderConfig(region);
    }

    // Update state, ensuring we don't keep active IDs for deleted regions
    // and automatically enabling newly downloaded regions.
    state = state.copyWith(
      availableProviders: currentProviders,
      activeOfflineIds: <String>{
        ...state.activeOfflineIds.where((id) => !toRemove.contains(id)),
        ...newlyAddedIds,
      }.toList(), // Use Set to avoid duplicates
    );
    _persistState();
  }

  void _persistState() {
    ref.read(layerPreferenceServiceProvider).saveLayers(
      global: state.activeGlobalIds,
      local: state.activeLocalIds,
      overlays: state.activeOverlayIds,
      offline: state.activeOfflineIds,
    );
  }

  void toggleGlobalLayer(String providerId) {
    if (state.availableProviders[providerId]?.category !=
        TileProviderCategory.global) {
      throw ArgumentError('Provider must be a global layer');
    }
    state = state.copyWith(
      activeGlobalIds: state.activeGlobalIds.contains(providerId)
          ? state.activeGlobalIds.where((id) => id != providerId).toList()
          : [providerId], // Can only have one global layer active
    );
    _persistState();
  }

  void toggleLocalLayer(String providerId) {
    final provider = state.availableProviders[providerId];
    if (provider?.category != TileProviderCategory.local) {
      throw ArgumentError('Provider must be a local layer');
    }

    state = state.copyWith(
      activeLocalIds: state.activeLocalIds.contains(providerId)
          ? state.activeLocalIds.where((id) => id != providerId).toList()
          : [providerId], // Can only have one local layer active
    );
    _persistState();
  }

  void toggleOverlay(String providerId) {
    if (state.availableProviders[providerId]?.category !=
        TileProviderCategory.overlay) {
      throw ArgumentError('Provider must be an overlay layer');
    }
    final currentIds = state.activeOverlayIds;
    state = state.copyWith(
      activeOverlayIds: currentIds.contains(providerId)
          ? currentIds.where((id) => id != providerId).toList()
          : [...currentIds, providerId],
    );
    _persistState();
  }

  void toggleOfflineLayer(String providerId) {
    if (state.availableProviders[providerId]?.category !=
        TileProviderCategory.offline) {
      throw ArgumentError('Provider must be an offline layer');
    }
    final currentIds = state.activeOfflineIds;
    state = state.copyWith(
      activeOfflineIds: currentIds.contains(providerId)
          ? currentIds.where((id) => id != providerId).toList()
          : [...currentIds, providerId],
    );
    _persistState();
  }

  List<TileLayer> getActiveLayers() {
    final layers = <TileLayer>[];

    // --- WEB IMPLEMENTATION ---
    // For web, we bypass the custom caching mechanism which is not web-compatible
    // and use the recommended CancellableNetworkTileProvider.
    if (kIsWeb) {
      final activeIds = [
        ...state.activeGlobalIds,
        ...state.activeLocalIds,
        ...state.activeOverlayIds,
      ];
      for (final id in activeIds) {
        final config = state.availableProviders[id];
        if (config == null) continue;
        // Vector-only configs are toggles for layers rendered elsewhere
        // (VectorDataLayer); they have no raster URL of their own.
        if (config.isVectorOnly) continue;

        final wms = config.wmsOptions;
        layers.add(TileLayer(
          tileProvider: NetworkTileProvider(
              headers: config.headers, silenceExceptions: true),
          urlTemplate: wms == null ? config.urlTemplate : null,
          wmsOptions: wms,
          minZoom: config.minZoom,
          maxZoom: 22, // Allow overzooming visually
          maxNativeZoom: config.maxZoom.toInt(),
          panBuffer: 2,
          evictErrorTileStrategy: EvictErrorTileStrategy.none,
          tileDisplay: TileDisplay.instantaneous(opacity: config.opacity),
        ));
      }
      return layers;
    }

    // --- NATIVE IMPLEMENTATION (The existing logic) ---
    final cacheServiceAsync = ref.read(cacheServiceProvider);
    // Subscribe so layers rebuild once the tile store is ready.
    ref.watch(tileStoreServiceProvider);
    final offlineNotifier = ref.read(offlineRegionsProvider.notifier);

    if (cacheServiceAsync is! AsyncData) {
      return [];
    }
    final cacheService = cacheServiceAsync.value;

    final activeIds = [
      ...state.activeGlobalIds,
      ...state.activeLocalIds,
      ...state.activeOfflineIds,
      ...state.activeOverlayIds,
    ];

    for (final id in activeIds) {
      final config = state.availableProviders[id];
      if (config == null) continue;
      if (config.isVectorOnly) continue;

      final wms = config.wmsOptions;
      final TileProvider? tileProvider;
      if (config.category == TileProviderCategory.offline) {
        tileProvider = offlineNotifier.createTileProvider(
            region: (config as OfflineRegionProviderConfig).region);
      } else {
        // For WMS sources the urlTemplate doubles as a stable provider-id
        // seed; flutter_map's TileLayer routes per-tile URL building through
        // wmsOptions, so the cache still keys by (providerId, coords) and
        // works end-to-end.
        tileProvider = cacheService?.createTileProvider(
          urlTemplate: config.urlTemplate,
          headers: config.headers,
        );
      }

      if (tileProvider != null) {
        layers.add(TileLayer(
          tileProvider: tileProvider,
          urlTemplate: wms == null ? config.urlTemplate : null,
          wmsOptions: wms,
          minZoom: config.minZoom,
          maxZoom: 22,
          maxNativeZoom: config.maxZoom.toInt(),
          panBuffer: 2,
          evictErrorTileStrategy: EvictErrorTileStrategy.none,
          tileDisplay: TileDisplay.instantaneous(opacity: config.opacity),
        ));
      }
    }
    return layers;
  }
}