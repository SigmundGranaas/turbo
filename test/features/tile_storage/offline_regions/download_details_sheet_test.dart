import 'package:flutter/material.dart';
import 'package:flutter_localizations/flutter_localizations.dart';
import 'package:flutter_map/flutter_map.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:latlong2/latlong.dart';
import 'package:turbo/features/tile_providers/data/tile_registry.dart';
import 'package:turbo/features/tile_providers/models/tile_provider_config.dart';
import 'package:turbo/features/tile_providers/models/tile_registry_state.dart';
import 'package:turbo/features/tile_storage/offline_regions/api.dart';
import 'package:turbo/features/tile_storage/offline_regions/widgets/download_details_sheet.dart';
import 'package:turbo/app/l10n/app_localizations.dart';

class _StubProvider extends TileProviderConfig {
  @override
  final String id;
  final String _name;
  @override
  final TileProviderCategory category;

  _StubProvider(this.id, this._name,
      [this.category = TileProviderCategory.global]);

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
  @override
  TileRegistryState build() {
    final osm = _StubProvider('osm', 'OpenStreetMap');
    final topo = _StubProvider('topo', 'Topo', TileProviderCategory.local);
    return TileRegistryState(
      availableProviders: {osm.id: osm, topo.id: topo},
      activeGlobalIds: const ['osm'],
      activeLocalIds: const [],
      activeOverlayIds: const [],
      activeOfflineIds: const [],
    );
  }
}

class _FakeOfflineNotifier extends OfflineRegionsNotifier {
  int createCallCount = 0;
  String? lastName;
  String? lastProviderId;

  @override
  Future<List<OfflineRegion>> build() async => const [];

  @override
  Future<void> createRegion({
    required String name,
    required LatLngBounds bounds,
    required int minZoom,
    required int maxZoom,
    required String urlTemplate,
    required String tileProviderId,
    required String tileProviderName,
  }) async {
    createCallCount++;
    lastName = name;
    lastProviderId = tileProviderId;
  }
}

Future<_FakeOfflineNotifier> _openSheet(WidgetTester tester) async {
  final offline = _FakeOfflineNotifier();
  final bounds = LatLngBounds(
    const LatLng(63.4, 10.3),
    const LatLng(63.5, 10.5),
  );
  await tester.pumpWidget(
    ProviderScope(
      overrides: [
        tileRegistryProvider.overrideWith(_FakeRegistry.new),
        offlineRegionsProvider.overrideWith(() => offline),
      ],
      child: MaterialApp(
        localizationsDelegates: const [
          AppLocalizations.delegate,
          GlobalMaterialLocalizations.delegate,
          GlobalWidgetsLocalizations.delegate,
          GlobalCupertinoLocalizations.delegate,
        ],
        supportedLocales: AppLocalizations.supportedLocales,
        home: Scaffold(
          body: Builder(
            builder: (ctx) => Center(
              child: ElevatedButton(
                child: const Text('open'),
                onPressed: () => showModalBottomSheet(
                  context: ctx,
                  isScrollControlled: true,
                  builder: (_) => DownloadDetailsSheet(bounds: bounds),
                ),
              ),
            ),
          ),
        ),
      ),
    ),
  );
  await tester.pumpAndSettle();
  await tester.tap(find.text('open'));
  await tester.pumpAndSettle();
  return offline;
}

void main() {
  group('DownloadDetailsSheet', () {
    testWidgets('opens with the default name pre-filled', (tester) async {
      await _openSheet(tester);
      // Localized default name "My Offline Map" is in the name field.
      expect(find.text('My Offline Map'), findsOneWidget);
    });

    testWidgets('shows the active provider as the default map source',
        (tester) async {
      await _openSheet(tester);
      expect(find.text('OpenStreetMap'), findsOneWidget);
    });

    testWidgets('validation: empty name blocks createRegion', (tester) async {
      final notifier = await _openSheet(tester);

      await tester.enterText(find.byType(TextFormField), '   ');
      await tester.pumpAndSettle();
      await tester.tap(find.text('Start Download'));
      await tester.pumpAndSettle();

      expect(notifier.createCallCount, 0);
    });

    testWidgets('valid form submits createRegion with the entered name',
        (tester) async {
      final notifier = await _openSheet(tester);

      await tester.enterText(find.byType(TextFormField), 'Trondheim local');
      await tester.pumpAndSettle();
      await tester.tap(find.text('Start Download'));
      await tester.pumpAndSettle();

      expect(notifier.createCallCount, 1);
      expect(notifier.lastName, 'Trondheim local');
      expect(notifier.lastProviderId, 'osm');
    });
  });
}
