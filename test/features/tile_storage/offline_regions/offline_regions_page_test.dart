import 'package:flutter/material.dart';
import 'package:flutter_localizations/flutter_localizations.dart';
import 'package:flutter_map/flutter_map.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:latlong2/latlong.dart';
import 'package:turbo/features/tile_storage/offline_regions/api.dart';
import 'package:turbo/l10n/app_localizations.dart';

/// Test double for [OfflineRegionsNotifier]. Records `deleteRegion` calls so
/// the delete-confirmation flow can be asserted on without touching SQLite,
/// the orchestrator, or platform channels.
class _FakeOfflineRegionsNotifier extends OfflineRegionsNotifier {
  _FakeOfflineRegionsNotifier(this._initial);
  final List<OfflineRegion> _initial;
  int deleteCallCount = 0;
  String? lastDeletedId;

  @override
  Future<List<OfflineRegion>> build() async => _initial;

  @override
  Future<void> deleteRegion(String regionId) async {
    deleteCallCount++;
    lastDeletedId = regionId;
    state = AsyncData(state.value!.where((r) => r.id != regionId).toList());
  }
}

OfflineRegion _region({
  String id = 'r1',
  String name = 'Trondheim',
  DownloadStatus status = DownloadStatus.completed,
}) {
  return OfflineRegion(
    id: id,
    name: name,
    bounds: LatLngBounds(
      const LatLng(63.4, 10.3),
      const LatLng(63.5, 10.5),
    ),
    minZoom: 10,
    maxZoom: 14,
    urlTemplate: 'https://example.com/{z}/{x}/{y}.png',
    tileProviderId: 'osm',
    tileProviderName: 'OSM',
    status: status,
    totalTiles: 100,
    downloadedTiles: 100,
  );
}

Future<_FakeOfflineRegionsNotifier> _pumpPage(
  WidgetTester tester, {
  required List<OfflineRegion> regions,
}) async {
  final fake = _FakeOfflineRegionsNotifier(regions);
  await tester.pumpWidget(
    ProviderScope(
      overrides: [
        offlineRegionsProvider.overrideWith(() => fake),
      ],
      child: const MaterialApp(
        localizationsDelegates: [
          AppLocalizations.delegate,
          GlobalMaterialLocalizations.delegate,
          GlobalWidgetsLocalizations.delegate,
          GlobalCupertinoLocalizations.delegate,
        ],
        supportedLocales: AppLocalizations.supportedLocales,
        home: OfflineRegionsPage(),
      ),
    ),
  );
  await tester.pumpAndSettle();
  return fake;
}

void main() {
  group('OfflineRegionsPage empty state', () {
    testWidgets(
        'shows the localized empty-state message when no regions exist',
        (tester) async {
      await _pumpPage(tester, regions: const []);
      expect(
        find.textContaining('No maps downloaded yet'),
        findsOneWidget,
      );
    });
  });

  group('OfflineRegionsPage delete flow (regression: bug fixed in this PR)',
      () {
    testWidgets(
        'tapping the delete icon shows the destructive dialog with the region name',
        (tester) async {
      await _pumpPage(tester, regions: [_region(name: 'Trondheim')]);

      expect(find.text('Trondheim'), findsOneWidget);

      await tester.tap(find.byIcon(Icons.delete_outline));
      await tester.pumpAndSettle();

      // Localized destructive dialog appears.
      expect(find.text('Delete Trondheim?'), findsOneWidget);
      expect(
          find.text('This will remove the offline map data from your device.'),
          findsOneWidget);
      expect(find.widgetWithText(FilledButton, 'Delete'), findsOneWidget);
      expect(find.widgetWithText(TextButton, 'Cancel'), findsOneWidget);
    });

    testWidgets('confirming the dialog deletes the region exactly once',
        (tester) async {
      final fake = await _pumpPage(tester, regions: [_region(id: 'r1')]);

      await tester.tap(find.byIcon(Icons.delete_outline));
      await tester.pumpAndSettle();
      await tester.tap(find.widgetWithText(FilledButton, 'Delete'));
      await tester.pumpAndSettle();

      expect(fake.deleteCallCount, 1,
          reason: 'deleteRegion must be called exactly once after confirmation');
      expect(fake.lastDeletedId, 'r1');
    });

    testWidgets(
        'cancelling the dialog does NOT call deleteRegion '
        '(regression: previous code popped after calling deleteRegion)',
        (tester) async {
      final fake = await _pumpPage(tester, regions: [_region(id: 'r1')]);

      await tester.tap(find.byIcon(Icons.delete_outline));
      await tester.pumpAndSettle();
      await tester.tap(find.widgetWithText(TextButton, 'Cancel'));
      await tester.pumpAndSettle();

      expect(fake.deleteCallCount, 0,
          reason: 'cancel must not delete');
      // The region is still listed.
      expect(find.text('Trondheim'), findsOneWidget);
    });

    testWidgets('barrier-dismiss does NOT call deleteRegion',
        (tester) async {
      final fake = await _pumpPage(tester, regions: [_region(id: 'r1')]);

      await tester.tap(find.byIcon(Icons.delete_outline));
      await tester.pumpAndSettle();
      await tester.tapAt(const Offset(20, 20)); // outside dialog
      await tester.pumpAndSettle();

      expect(fake.deleteCallCount, 0);
    });
  });

  group('OfflineRegionsPage list rendering', () {
    testWidgets('renders one Card per region with status avatar',
        (tester) async {
      await _pumpPage(tester, regions: [
        _region(id: 'a', name: 'Region A'),
        _region(id: 'b', name: 'Region B'),
      ]);
      expect(find.text('Region A'), findsOneWidget);
      expect(find.text('Region B'), findsOneWidget);
      expect(find.byType(CircleAvatar), findsNWidgets(2));
    });
  });
}
