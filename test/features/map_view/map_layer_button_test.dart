import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:turbo/features/map_view/widgets/buttons/map_layer_button.dart';
import 'package:turbo/features/tile_providers/data/tile_registry.dart';
import 'package:turbo/features/tile_providers/models/tile_provider_config.dart';
import 'package:turbo/features/tile_providers/models/tile_registry_state.dart';

import '../../helpers/pump_app.dart';

class _StubProvider extends TileProviderConfig {
  @override
  final String id;
  final String _name;
  @override
  final TileProviderCategory category;

  _StubProvider(this.id, this._name, this.category);

  @override
  String name(BuildContext context) => _name;
  @override
  String description(BuildContext context) => '';
  @override
  String get attributions => '';
  @override
  String get urlTemplate => 'https://x/{z}/{x}/{y}.png';
}

class _FakeRegistry extends TileRegistry {
  final List<String> activeOffline;
  _FakeRegistry({this.activeOffline = const []});

  @override
  TileRegistryState build() {
    final osm = _StubProvider('osm', 'OSM', TileProviderCategory.global);
    final topo = _StubProvider('topo', 'Topo', TileProviderCategory.local);
    final saved = _StubProvider('saved-1', 'Trondheim',
        TileProviderCategory.offline);
    final providers = {osm.id: osm, topo.id: topo};
    if (activeOffline.isNotEmpty) providers[saved.id] = saved;
    return TileRegistryState(
      availableProviders: providers,
      activeGlobalIds: const ['osm'],
      activeLocalIds: const ['topo'],
      activeOverlayIds: const [],
      activeOfflineIds: activeOffline,
    );
  }
}

Future<void> _pumpSheet(
  WidgetTester tester, {
  List<String> activeOffline = const [],
}) async {
  await pumpTestApp(
    tester,
    const LayerSelectionSheet(),
    overrides: [
      tileRegistryProvider
          .overrideWith(() => _FakeRegistry(activeOffline: activeOffline)),
    ],
  );
}

void main() {
  group('LayerSelectionSheet section rendering', () {
    testWidgets('renders global, local, and data sections',
        (tester) async {
      await _pumpSheet(tester);

      // Section headers (these strings are localized to English by default).
      expect(find.text('Global Maps'), findsOneWidget);
      expect(find.text('Norwegian Maps'), findsOneWidget);
      expect(find.text('Data'), findsOneWidget);
      expect(find.text('Offline Maps'), findsOneWidget);

      // The two providers we stubbed are visible.
      expect(find.text('OSM'), findsOneWidget);
      expect(find.text('Topo'), findsOneWidget);
    });

    testWidgets('offline section empty state shows the localized message and '
        'the Manage/Download buttons', (tester) async {
      await _pumpSheet(tester);
      expect(
          find.textContaining('No maps downloaded yet'), findsOneWidget);
      expect(find.text('Manage'), findsOneWidget);
      expect(find.text('Download'), findsOneWidget);
    });

    testWidgets(
        'offline section with one downloaded region shows it and the '
        'Manage/Download buttons', (tester) async {
      await _pumpSheet(tester, activeOffline: const ['saved-1']);
      expect(find.text('Trondheim'), findsOneWidget);
      expect(find.text('Manage'), findsOneWidget);
      expect(find.text('Download'), findsOneWidget);
    });

    testWidgets('Markers and Paths SwitchListTiles render with switches',
        (tester) async {
      await _pumpSheet(tester);
      expect(find.byType(SwitchListTile), findsNWidgets(2));
    });
  });
}
