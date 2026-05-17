import 'package:flutter/material.dart';
import 'package:flutter_riverpod/misc.dart' show Override;
import 'package:flutter_test/flutter_test.dart';
import 'package:shared_preferences/shared_preferences.dart';
import 'package:turbo/features/map_view/widgets/buttons/map_layer_button.dart';
import 'package:turbo/features/tile_providers/data/layer_preference_service.dart';
import 'package:turbo/features/tile_providers/widgets/add_custom_map_page.dart';
import 'package:turbo/features/tile_storage/offline_regions/data/offline_regions_notifier.dart';
import 'package:turbo/features/tile_storage/offline_regions/models/offline_region.dart';

import '../../helpers/fakes/fake_layer_preference_service.dart';
import '../../helpers/pump_app.dart';

class _StubOfflineRegions extends OfflineRegionsNotifier {
  _StubOfflineRegions();
  @override
  Future<List<OfflineRegion>> build() async => const [];
}

/// Pumps a Scaffold whose body is a button that opens the LayerSelectionSheet
/// — closing the sheet and pushing AddCustomMapPage involves the host
/// Navigator, so a button-host is friendlier than mounting the sheet directly.
Future<void> _pumpLayerHost(
  WidgetTester tester, {
  List<Override> overrides = const [],
}) async {
  await pumpTestApp(
    tester,
    Builder(
      builder: (ctx) => Center(
        child: ElevatedButton(
          child: const Text('open-sheet'),
          onPressed: () => showModalBottomSheet<void>(
            context: ctx,
            isScrollControlled: true,
            useSafeArea: true,
            builder: (_) => const LayerSelectionSheet(),
          ),
        ),
      ),
    ),
    resetSharedPrefs: false,
    overrides: overrides,
  );
  await tester.tap(find.text('open-sheet'));
  await tester.pumpAndSettle();
}

void main() {
  group('Add custom map end-to-end', () {
    testWidgets(
        'tapping Add custom map closes the layer sheet and pushes the '
        'AddCustomMapPage; submitting a valid form persists the provider',
        (tester) async {
      SharedPreferences.setMockInitialValues({});
      final prefs = FakeLayerPreferenceService(initialLocal: ['topo']);

      await _pumpLayerHost(
        tester,
        overrides: [
          layerPreferenceServiceProvider.overrideWithValue(prefs),
          offlineRegionsProvider.overrideWith(() => _StubOfflineRegions()),
        ],
      );

      // Scroll to surface the "Add custom map…" tile inside the sheet and
      // tap it. The tile is at the bottom of the sheet's scroll view.
      await tester.dragUntilVisible(
        find.text('Add custom map…'),
        find.byType(SingleChildScrollView),
        const Offset(0, -200),
      );
      await tester.tap(find.text('Add custom map…'));
      await tester.pumpAndSettle();

      // Behavior: the sheet closes and the new page is on top.
      expect(find.byType(LayerSelectionSheet), findsNothing);
      expect(find.byType(AddCustomMapPage), findsOneWidget);
      // The page has an AppBar with the title.
      expect(find.widgetWithText(AppBar, 'Add custom map…'), findsOneWidget);

      // Fill in a valid name + URL.
      await tester.enterText(
          find.byType(TextFormField).first, 'My TMS Server');
      await tester.enterText(
          find.byType(TextFormField).at(1),
          'https://tiles.example.com/{z}/{x}/{y}.png');

      // Save via the AppBar action.
      await tester.tap(find.widgetWithText(TextButton, 'Add'));
      await tester.pumpAndSettle();

      // Behavior: page popped.
      expect(find.byType(AddCustomMapPage), findsNothing);

      // Behavior: provider persisted under the SharedPreferences key.
      final stored = await SharedPreferences.getInstance();
      final raw = stored.getString('custom_tile_providers');
      expect(raw, isNotNull);
      expect(raw, contains('My TMS Server'));
      expect(raw, contains(r'tiles.example.com'));
    });

    testWidgets(
        'invalid URL template shows the localized validator error and the '
        'page stays open without writing to prefs', (tester) async {
      SharedPreferences.setMockInitialValues({});
      final prefs = FakeLayerPreferenceService(initialLocal: ['topo']);

      await _pumpLayerHost(
        tester,
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

      await tester.tap(find.widgetWithText(TextButton, 'Add'));
      await tester.pumpAndSettle();

      // Validator error renders below the field; page is still mounted.
      expect(
          find.textContaining(
              'Invalid URL template. It must contain {z}, {x}, and {y}.'),
          findsOneWidget);
      expect(find.byType(AddCustomMapPage), findsOneWidget);

      // Nothing persisted.
      final stored = await SharedPreferences.getInstance();
      expect(stored.containsKey('custom_tile_providers'), isFalse);
    });

    testWidgets(
        'AppBar back button cancels without writing to prefs',
        (tester) async {
      SharedPreferences.setMockInitialValues({});
      final prefs = FakeLayerPreferenceService(initialLocal: ['topo']);

      await _pumpLayerHost(
        tester,
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

      // Type something then back out.
      await tester.enterText(
          find.byType(TextFormField).first, 'Half-typed');
      await tester.tap(find.byTooltip('Back'));
      await tester.pumpAndSettle();

      // Page closed; nothing persisted.
      expect(find.byType(AddCustomMapPage), findsNothing);
      final stored = await SharedPreferences.getInstance();
      expect(stored.containsKey('custom_tile_providers'), isFalse);
    });
  });
}
