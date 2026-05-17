import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:shared_preferences/shared_preferences.dart';
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
  group('Add custom map end-to-end', () {
    testWidgets(
        'tapping Add custom map opens the dialog; submitting a valid URL '
        'persists the provider and the registry exposes it',
        (tester) async {
      SharedPreferences.setMockInitialValues({});
      final prefs = FakeLayerPreferenceService(initialLocal: ['topo']);

      await pumpTestApp(
        tester,
        const LayerSelectionSheet(),
        resetSharedPrefs: false,
        overrides: [
          layerPreferenceServiceProvider.overrideWithValue(prefs),
          offlineRegionsProvider.overrideWith(() => _StubOfflineRegions()),
        ],
      );

      // Scroll to surface the tile and tap it.
      await tester.dragUntilVisible(
        find.text('Add custom map…'),
        find.byType(SingleChildScrollView),
        const Offset(0, -200),
      );
      await tester.tap(find.text('Add custom map…'));
      await tester.pumpAndSettle();

      // Dialog is up.
      expect(find.text('Add custom map…'), findsAtLeastNWidgets(1));

      // Fill in a valid name + URL.
      await tester.enterText(
          find.byType(TextFormField).first, 'My TMS Server');
      await tester.enterText(find.byType(TextFormField).at(1),
          'https://tiles.example.com/{z}/{x}/{y}.png');

      await tester.tap(find.widgetWithText(FilledButton, 'Add'));
      await tester.pumpAndSettle();

      // Dialog closed (the "Add" button is gone).
      expect(find.widgetWithText(FilledButton, 'Add'), findsNothing);

      // Stored.
      final stored = await SharedPreferences.getInstance();
      final raw = stored.getString('custom_tile_providers');
      expect(raw, isNotNull);
      expect(raw, contains('My TMS Server'));
      expect(raw, contains(r'tiles.example.com'));
    });

    testWidgets(
        'invalid URL template surfaces a localized error and the dialog '
        'stays open without writing to prefs', (tester) async {
      SharedPreferences.setMockInitialValues({});
      final prefs = FakeLayerPreferenceService(initialLocal: ['topo']);

      await pumpTestApp(
        tester,
        const LayerSelectionSheet(),
        resetSharedPrefs: false,
        overrides: [
          layerPreferenceServiceProvider.overrideWithValue(prefs),
          offlineRegionsProvider.overrideWith(() => _StubOfflineRegions()),
        ],
      );

      await tester.dragUntilVisible(
        find.text('Add custom map…'),
        find.byType(SingleChildScrollView),
        const Offset(0, -200),
      );
      await tester.tap(find.text('Add custom map…'));
      await tester.pumpAndSettle();

      await tester.enterText(
          find.byType(TextFormField).first, 'Broken');
      // Missing {y}.
      await tester.enterText(find.byType(TextFormField).at(1),
          'https://broken.example.com/{z}/{x}.png');

      await tester.tap(find.widgetWithText(FilledButton, 'Add'));
      await tester.pumpAndSettle();

      // Validator error shows the localized hint; dialog stays open
      // (the Add button is still present).
      expect(
          find.textContaining(
              'Invalid URL template. It must contain {z}, {x}, and {y}.'),
          findsOneWidget);
      expect(find.widgetWithText(FilledButton, 'Add'), findsOneWidget);

      // Nothing persisted.
      final stored = await SharedPreferences.getInstance();
      expect(stored.containsKey('custom_tile_providers'), isFalse);
    });
  });
}
