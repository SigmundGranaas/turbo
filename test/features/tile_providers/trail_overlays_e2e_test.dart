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

/// End-to-end coverage for the four trail subtypes registered under the
/// Nasjonal turbase umbrella. Each subtype must:
///  1. Show up as an independently-toggleable row in the layer picker.
///  2. Persist its overlay id when tapped.
///  3. Leave the other subtypes alone (no all-or-nothing behaviour).
///
/// This is the contract the user reported as broken in the previous round,
/// so we lock it down with widget-level assertions rather than just
/// asserting on the registry state.
void main() {
  group('Trail subtype overlays — layer picker', () {
    const subtypes = <_Subtype>[
      _Subtype('Hiking trails', 'trails_foot'),
      _Subtype('Ski tracks', 'trails_ski'),
      _Subtype('Bike routes', 'trails_bike'),
      _Subtype('Other routes', 'trails_other'),
    ];

    for (final subtype in subtypes) {
      testWidgets('"${subtype.label}" appears as its own overlay row',
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
        await tester.ensureVisible(find.text(subtype.label));
        await tester.pumpAndSettle();
        expect(find.text(subtype.label), findsOneWidget);
      });

      testWidgets(
          'tapping "${subtype.label}" persists only its overlay id; the '
          'other three stay off', (tester) async {
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
        await tester.ensureVisible(find.text(subtype.label));
        await tester.pumpAndSettle();
        await tester.tap(find.text(subtype.label));
        await tester.pumpAndSettle();

        expect(prefs.overlays, [subtype.overlayId]);
        for (final other in subtypes) {
          if (other.overlayId == subtype.overlayId) continue;
          expect(prefs.overlays, isNot(contains(other.overlayId)),
              reason:
                  'Toggling "${subtype.label}" must not silently enable '
                  '"${other.label}".');
        }
      });
    }

    testWidgets('two subtypes can be active simultaneously without '
        'overwriting each other', (tester) async {
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

      await tester.ensureVisible(find.text('Hiking trails'));
      await tester.pumpAndSettle();
      await tester.tap(find.text('Hiking trails'));
      await tester.pumpAndSettle();

      await tester.ensureVisible(find.text('Ski tracks'));
      await tester.pumpAndSettle();
      await tester.tap(find.text('Ski tracks'));
      await tester.pumpAndSettle();

      expect(prefs.overlays, containsAll(['trails_foot', 'trails_ski']));
      expect(prefs.overlays, isNot(contains('trails_bike')));
      expect(prefs.overlays, isNot(contains('trails_other')));
    });
  });
}

class _Subtype {
  final String label;
  final String overlayId;
  const _Subtype(this.label, this.overlayId);
}
