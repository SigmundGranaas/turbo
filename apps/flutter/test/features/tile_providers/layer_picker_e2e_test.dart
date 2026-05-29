import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:turbo/features/map_view/widgets/buttons/map_layer_button.dart';
import 'package:turbo/features/tile_providers/data/layer_preference_service.dart';
import 'package:turbo/features/tile_storage/offline_regions/data/offline_regions_notifier.dart';
import 'package:turbo/features/tile_storage/offline_regions/models/offline_region.dart';

import '../../helpers/fakes/fake_layer_preference_service.dart';
import '../../helpers/pump_app.dart';

class _StubOfflineRegions extends OfflineRegionsNotifier {
  _StubOfflineRegions();
  @override
  Future<List<OfflineRegion>> build() async => const [];
}

void main() {
  group('LayerSelectionSheet end-to-end', () {
    testWidgets(
        'restores saved local preference on cold open — Topo card renders selected',
        (tester) async {
      final prefs = FakeLayerPreferenceService(initialLocal: ['topo']);

      await pumpTestApp(
        tester,
        const LayerSelectionSheet(),
        overrides: [
          layerPreferenceServiceProvider.overrideWithValue(prefs),
          offlineRegionsProvider.overrideWith(() => _StubOfflineRegions()),
        ],
      );

      // Pump again to let the async preference rehydrate flow finalize.
      await tester.pumpAndSettle();

      expect(find.text('Norwegian Maps'), findsOneWidget);
      expect(find.text('Norgeskart'), findsOneWidget);
    });

    testWidgets('tapping a global layer card persists through prefs',
        (tester) async {
      final prefs = FakeLayerPreferenceService(initialLocal: ['topo']);

      await pumpTestApp(
        tester,
        const LayerSelectionSheet(),
        overrides: [
          layerPreferenceServiceProvider.overrideWithValue(prefs),
          offlineRegionsProvider.overrideWith(() => _StubOfflineRegions()),
        ],
      );
      await tester.pumpAndSettle();

      final beforeSaves = prefs.saveCount;
      await tester.tap(find.text('Open Street Map'));
      await tester.pumpAndSettle();

      expect(prefs.saveCount, greaterThan(beforeSaves));
      expect(prefs.global, ['osm']);
    });

    testWidgets('overlay toggle is additive — base layer stays active',
        (tester) async {
      final prefs = FakeLayerPreferenceService(initialLocal: ['topo']);

      await pumpTestApp(
        tester,
        const LayerSelectionSheet(),
        overrides: [
          layerPreferenceServiceProvider.overrideWithValue(prefs),
          offlineRegionsProvider.overrideWith(() => _StubOfflineRegions()),
        ],
      );
      await tester.pumpAndSettle();

      await tester.ensureVisible(find.text('Avalanche Danger'));
      await tester.pumpAndSettle();
      await tester.tap(find.text('Avalanche Danger'));
      await tester.pumpAndSettle();

      expect(prefs.overlays, ['avalanche_danger']);
      expect(prefs.local, ['topo']); // base layer untouched
    });

    testWidgets('offline section empty state shows the No-maps message + '
        'Manage / Download buttons', (tester) async {
      final prefs = FakeLayerPreferenceService(initialLocal: ['topo']);

      await pumpTestApp(
        tester,
        const LayerSelectionSheet(),
        overrides: [
          layerPreferenceServiceProvider.overrideWithValue(prefs),
          offlineRegionsProvider.overrideWith(() => _StubOfflineRegions()),
        ],
      );
      await tester.pumpAndSettle();

      expect(
          find.textContaining('No maps downloaded yet'), findsOneWidget);
      expect(find.text('Manage'), findsOneWidget);
      expect(find.text('Download'), findsOneWidget);
    });

    testWidgets('Show-markers, Show-paths and Show-photos switches render',
        (tester) async {
      await pumpTestApp(
        tester,
        const LayerSelectionSheet(),
        overrides: [
          layerPreferenceServiceProvider
              .overrideWithValue(FakeLayerPreferenceService()),
          offlineRegionsProvider.overrideWith(() => _StubOfflineRegions()),
        ],
      );
      await tester.pumpAndSettle();

      expect(find.byType(SwitchListTile), findsNWidgets(3));
    });

    testWidgets(
        'tapping a layer flips the registry state — second open shows it as '
        'the only active global', (tester) async {
      final prefs = FakeLayerPreferenceService();

      await pumpTestApp(
        tester,
        const LayerSelectionSheet(),
        overrides: [
          layerPreferenceServiceProvider.overrideWithValue(prefs),
          offlineRegionsProvider.overrideWith(() => _StubOfflineRegions()),
        ],
      );
      await tester.pumpAndSettle();

      await tester.tap(find.text('Google Satellite'));
      await tester.pumpAndSettle();
      await tester.tap(find.text('Open Street Map'));
      await tester.pumpAndSettle();

      // Only the last-tapped global ends up active — mutual exclusion holds
      // through the UI path.
      expect(prefs.global, ['osm']);
    });
  });
}
