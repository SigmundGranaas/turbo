import 'package:flutter_map/flutter_map.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:latlong2/latlong.dart';
import 'package:turbo/features/tile_providers/data/layer_preference_service.dart';
import 'package:turbo/features/tile_providers/data/tile_registry.dart';
import 'package:turbo/features/tile_providers/models/tile_provider_config.dart';
import 'package:turbo/features/tile_storage/offline_regions/data/offline_regions_notifier.dart';
import 'package:turbo/features/tile_storage/offline_regions/models/offline_region.dart';

import '../../helpers/fakes/fake_layer_preference_service.dart';
import '../../helpers/wait_for.dart';

/// Stubs [OfflineRegionsNotifier] so the registry's `ref.listen` has a
/// predictable AsyncNotifier to subscribe to. Tests drive [emit] to push new
/// region lists through.
class _StubOfflineRegions extends OfflineRegionsNotifier {
  _StubOfflineRegions({this.initial = const []});
  final List<OfflineRegion> initial;

  @override
  Future<List<OfflineRegion>> build() async => initial;

  void emit(List<OfflineRegion> regions) {
    state = AsyncData(regions);
  }
}

OfflineRegion _region(String id, {String name = 'Region'}) => OfflineRegion(
      id: id,
      name: name,
      bounds: LatLngBounds(
        const LatLng(59.0, 10.0),
        const LatLng(60.0, 11.0),
      ),
      minZoom: 1,
      maxZoom: 12,
      urlTemplate: 'https://x/{z}/{x}/{y}.png',
      tileProviderId: 'osm',
      tileProviderName: 'OSM',
    );

/// Builds a container with the registry wired to a [FakeLayerPreferenceService]
/// and a stubbed offline regions notifier. Returns the container, the fake,
/// and the stub for later manipulation.
({
  ProviderContainer container,
  FakeLayerPreferenceService prefs,
  _StubOfflineRegions offline,
}) makeRegistryContainer({
  FakeLayerPreferenceService? prefs,
  List<OfflineRegion> initialOffline = const [],
}) {
  final fakePrefs = prefs ?? FakeLayerPreferenceService();
  final stubOffline = _StubOfflineRegions(initial: initialOffline);
  final container = ProviderContainer(overrides: [
    layerPreferenceServiceProvider.overrideWithValue(fakePrefs),
    offlineRegionsProvider.overrideWith(() => stubOffline),
  ]);
  addTearDown(container.dispose);
  return (container: container, prefs: fakePrefs, offline: stubOffline);
}

void main() {
  group('TileRegistry initial state', () {
    test('registers the six built-in providers synchronously', () {
      final c = makeRegistryContainer();
      final state = c.container.read(tileRegistryProvider);

      expect(state.availableProviders.keys, {
        'topo',
        'sjokart',
        'osm',
        'gs',
        'avalanche_danger',
        'openseamap',
      });
      expect(
          state.availableProviders['topo']!.category,
          TileProviderCategory.local);
      expect(
          state.availableProviders['sjokart']!.category,
          TileProviderCategory.local);
      expect(
          state.availableProviders['osm']!.category,
          TileProviderCategory.global);
      expect(
          state.availableProviders['avalanche_danger']!.category,
          TileProviderCategory.overlay);
      expect(
          state.availableProviders['openseamap']!.category,
          TileProviderCategory.overlay);
    });

    test(
        'applies the topo default on first launch when no preferences are saved',
        () async {
      final c = makeRegistryContainer();
      // Subscribe so the async preferences-load actually fires.
      c.container.read(tileRegistryProvider);

      final state = await waitForState<dynamic>(
        c.container,
        tileRegistryProvider,
        (s) => s.activeLocalIds.contains('topo'),
      );

      expect(state.activeLocalIds, ['topo']);
      expect(state.activeGlobalIds, isEmpty);
      // Default application persisted through the toggle call.
      expect(c.prefs.saveCount, greaterThan(0));
    });

    test('restores saved local + global preferences on cold start', () async {
      final c = makeRegistryContainer(
        prefs: FakeLayerPreferenceService(
          initialGlobal: ['osm'],
          initialLocal: ['topo'],
          initialOverlays: ['avalanche_danger'],
        ),
      );
      c.container.read(tileRegistryProvider);

      final state = await waitForState<dynamic>(
        c.container,
        tileRegistryProvider,
        (s) => s.activeGlobalIds.contains('osm'),
      );

      expect(state.activeGlobalIds, ['osm']);
      expect(state.activeLocalIds, ['topo']);
      expect(state.activeOverlayIds, ['avalanche_danger']);
    });
  });

  group('TileRegistry.toggleGlobalLayer', () {
    test('activating a global layer replaces the previous one', () {
      final c = makeRegistryContainer();
      final notifier = c.container.read(tileRegistryProvider.notifier);

      notifier.toggleGlobalLayer('osm');
      expect(c.container.read(tileRegistryProvider).activeGlobalIds, ['osm']);

      // 'gs' (Google Satellite) replaces 'osm' — only one global at a time.
      notifier.toggleGlobalLayer('gs');
      expect(c.container.read(tileRegistryProvider).activeGlobalIds, ['gs']);

      // Toggling the active one off clears it.
      notifier.toggleGlobalLayer('gs');
      expect(
          c.container.read(tileRegistryProvider).activeGlobalIds, isEmpty);
    });

    test('rejects providers from a different category', () {
      final c = makeRegistryContainer();
      final notifier = c.container.read(tileRegistryProvider.notifier);

      expect(() => notifier.toggleGlobalLayer('topo'),
          throwsA(isA<ArgumentError>()));
      expect(() => notifier.toggleGlobalLayer('avalanche_danger'),
          throwsA(isA<ArgumentError>()));
    });

    test('persists state on every toggle', () {
      final c = makeRegistryContainer();
      final notifier = c.container.read(tileRegistryProvider.notifier);
      final startingSaves = c.prefs.saveCount;

      notifier.toggleGlobalLayer('osm');
      expect(c.prefs.saveCount, startingSaves + 1);
      expect(c.prefs.global, ['osm']);
    });
  });

  group('TileRegistry.toggleLocalLayer', () {
    test('mutually exclusive — replaces previous local layer', () {
      final c = makeRegistryContainer();
      final notifier = c.container.read(tileRegistryProvider.notifier);

      notifier.toggleLocalLayer('topo');
      expect(c.container.read(tileRegistryProvider).activeLocalIds, ['topo']);

      // Toggling same id off.
      notifier.toggleLocalLayer('topo');
      expect(
          c.container.read(tileRegistryProvider).activeLocalIds, isEmpty);
    });

    test('rejects wrong-category providers', () {
      final c = makeRegistryContainer();
      final notifier = c.container.read(tileRegistryProvider.notifier);
      expect(() => notifier.toggleLocalLayer('osm'),
          throwsA(isA<ArgumentError>()));
    });
  });

  group('TileRegistry.toggleOverlay', () {
    test('additive — overlays accumulate', () {
      final c = makeRegistryContainer();
      final notifier = c.container.read(tileRegistryProvider.notifier);

      notifier.toggleOverlay('avalanche_danger');
      expect(c.container.read(tileRegistryProvider).activeOverlayIds,
          ['avalanche_danger']);

      // Toggling again removes it.
      notifier.toggleOverlay('avalanche_danger');
      expect(c.container.read(tileRegistryProvider).activeOverlayIds,
          isEmpty);
    });

    test('rejects non-overlay providers', () {
      final c = makeRegistryContainer();
      final notifier = c.container.read(tileRegistryProvider.notifier);
      expect(() => notifier.toggleOverlay('osm'),
          throwsA(isA<ArgumentError>()));
    });
  });

  group('TileRegistry.toggleOfflineLayer', () {
    test('additive — offline layers accumulate', () async {
      final c = makeRegistryContainer(initialOffline: [
        _region('region-a'),
        _region('region-b'),
      ]);
      // Trigger build & let _syncOfflineProviders register the two regions.
      c.container.read(tileRegistryProvider);
      await waitForState<dynamic>(
        c.container,
        tileRegistryProvider,
        (s) => s.availableProviders.containsKey('region-a'),
      );

      final notifier = c.container.read(tileRegistryProvider.notifier);
      // Newly downloaded regions auto-activate, so toggling once removes one.
      notifier.toggleOfflineLayer('region-a');
      expect(
          c.container.read(tileRegistryProvider).activeOfflineIds
              .contains('region-a'),
          isFalse);
      expect(
          c.container.read(tileRegistryProvider).activeOfflineIds
              .contains('region-b'),
          isTrue);
    });
  });

  group('TileRegistry._syncOfflineProviders (via offlineRegionsProvider)', () {
    test('newly downloaded regions auto-activate', () async {
      final c = makeRegistryContainer();
      c.container.read(tileRegistryProvider);
      // Let the initial async (default-topo) settle first.
      await waitForState<dynamic>(
        c.container,
        tileRegistryProvider,
        (s) => s.activeLocalIds.contains('topo'),
      );

      c.offline.emit([_region('new-region')]);
      await waitForState<dynamic>(
        c.container,
        tileRegistryProvider,
        (s) => s.activeOfflineIds.contains('new-region'),
      );

      final state = c.container.read(tileRegistryProvider);
      expect(state.availableProviders.containsKey('new-region'), isTrue);
      expect(
          state.availableProviders['new-region']!.category,
          TileProviderCategory.offline);
    });

    test(
        'deleted regions are pruned from availableProviders and activeOfflineIds',
        () async {
      final c = makeRegistryContainer(initialOffline: [
        _region('region-a'),
        _region('region-b'),
      ]);
      c.container.read(tileRegistryProvider);
      await waitForState<dynamic>(
        c.container,
        tileRegistryProvider,
        (s) => s.activeOfflineIds.contains('region-a') &&
            s.activeOfflineIds.contains('region-b'),
      );

      c.offline.emit([_region('region-b')]);
      await waitForState<dynamic>(
        c.container,
        tileRegistryProvider,
        (s) => !s.availableProviders.containsKey('region-a'),
      );

      final state = c.container.read(tileRegistryProvider);
      expect(state.availableProviders.containsKey('region-b'), isTrue);
      expect(state.activeOfflineIds.contains('region-a'), isFalse);
      expect(state.activeOfflineIds.contains('region-b'), isTrue);
    });

    test('sync persists state through the preference service', () async {
      final c = makeRegistryContainer();
      c.container.read(tileRegistryProvider);
      await waitForState<dynamic>(
        c.container,
        tileRegistryProvider,
        (s) => s.activeLocalIds.contains('topo'),
      );
      final beforeSaves = c.prefs.saveCount;

      c.offline.emit([_region('new-region')]);
      await waitForState<dynamic>(
        c.container,
        tileRegistryProvider,
        (s) => s.activeOfflineIds.contains('new-region'),
      );

      expect(c.prefs.saveCount, greaterThan(beforeSaves));
      expect(c.prefs.offline, contains('new-region'));
    });
  });
}
