import 'package:flutter/material.dart';
import 'package:flutter_localizations/flutter_localizations.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:latlong2/latlong.dart';
import 'package:turbo/features/collections/api.dart';
import 'package:turbo/core/location/location_state.dart';
import 'package:turbo/features/markers/api.dart';
import 'package:turbo/features/navigation/api.dart';
import 'package:turbo/features/weather/api.dart';
import 'package:turbo/app/l10n/app_localizations.dart';

/// Returns no fix, so the Navigate action takes its straight-line fallback
/// (route-from-location needs a start). Also avoids leaking the real
/// geolocation stream's timer into the test.
class _NullLocation extends LocationState {
  @override
  Future<LatLng?> build() async => null;
}

class _NoopPhotoStore implements MarkerPhotoDataStore {
  @override
  Future<void> init() async {}
  @override
  Future<void> insert(MarkerPhoto photo) async {}
  @override
  Future<List<MarkerPhoto>> getByMarker(String markerUuid) async => [];
  @override
  Future<MarkerPhoto?> getByUuid(String uuid) async => null;
  @override
  Future<void> delete(String uuid) async {}
  @override
  Future<void> deleteAllForMarker(String markerUuid) async {}
}

/// Minimal fake repository that records delete calls. The sheet only needs
/// `deleteMarker`; the rest of [LocationRepository] is unused in this flow.
class _FakeRepo extends LocationRepository {
  int deleteCallCount = 0;
  String? lastDeletedUuid;
  bool shouldFail = false;

  @override
  AsyncValue<List<Marker>> build() => const AsyncData([]);

  @override
  Future<void> deleteMarker(String uuid) async {
    deleteCallCount++;
    lastDeletedUuid = uuid;
    if (shouldFail) throw Exception('boom');
  }
}

class _FakeCollectionRepo extends CollectionRepository {
  @override
  AsyncValue<CollectionRepositoryState> build() =>
      const AsyncData(CollectionRepositoryState.empty());

  @override
  Future<void> handleItemDeleted(CollectionItemRef ref) async {}
}

class _StubFetcher implements WeatherFetcher {
  @override
  YrAtmosphericService get atmospheric => throw UnimplementedError();
  @override
  YrOceanService get ocean => throw UnimplementedError();
  @override
  YrSunriseService get sunrise => throw UnimplementedError();
  @override
  MetAlertsService get alerts => throw UnimplementedError();

  @override
  Future<WeatherForecast> fetch(
    LatLng position, {
    WeatherForecast? previous,
  }) async {
    final now = DateTime.now().toUtc();
    return WeatherForecast(
      position: position,
      fetchedAt: now,
      atmosphericExpiresAt: now.add(const Duration(minutes: 30)),
      marineExpiresAt: null,
      atmosphericLastModified: null,
      marineLastModified: null,
      atmospheric: [
        AtmosphericPoint(
          timeUtc: now,
          airTemperatureC: 12.0,
          windSpeedMs: 3.0,
          windFromDeg: 180,
          humidity: null,
          pressureHpa: null,
          cloudCoverPercent: null,
          uvIndex: null,
          precipitation1hMm: 0.0,
          symbol1h: null,
          symbol6h: null,
          symbol12h: null,
        ),
      ],
      marine: const [],
    );
  }
}

Marker _marker() => Marker(
      uuid: 'm1',
      title: 'My Pin',
      description: 'Some place',
      position: const LatLng(63.4, 10.4),
    );

Future<_FakeRepo> _openSheet(WidgetTester tester, {Marker? marker}) async {
  final repo = _FakeRepo();
  await _openSheetWith(tester, marker: marker, repo: repo);
  return repo;
}

/// Variant that returns the [ProviderContainer] so tests can inspect or seed
/// providers (e.g. start a navigation before opening the sheet).
Future<ProviderContainer> _openSheetWith(
  WidgetTester tester, {
  Marker? marker,
  _FakeRepo? repo,
}) async {
  final fakeRepo = repo ?? _FakeRepo();
  final m = marker ?? _marker();
  final container = ProviderContainer(
    overrides: [
      locationRepositoryProvider.overrideWith(() => fakeRepo),
      locationStateProvider.overrideWith(() => _NullLocation()),
      collectionRepositoryProvider.overrideWith(() => _FakeCollectionRepo()),
      localMarkerPhotoDataStoreProvider
          .overrideWith((ref) async => _NoopPhotoStore()),
      // Weather block is included in MarkerInfoSheet; stub the fetcher so
      // existing tests don't make network calls.
      weatherFetcherProvider.overrideWith((ref) => _StubFetcher()),
    ],
  );
  addTearDown(container.dispose);

  await tester.pumpWidget(
    UncontrolledProviderScope(
      container: container,
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
                onPressed: () => showModalBottomSheet<MarkerInfoResult>(
                  context: ctx,
                  // Mirrors production callers (e.g. viewport_marker_layer):
                  // the sheet contains a WeatherSection that grows beyond
                  // the default half-screen modal height.
                  isScrollControlled: true,
                  useSafeArea: true,
                  builder: (_) => MarkerInfoSheet(marker: m),
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
  return container;
}

void main() {
  group('MarkerInfoSheet header', () {
    testWidgets('renders the title, coordinates, and description',
        (tester) async {
      await _openSheet(tester);
      expect(find.text('My Pin'), findsOneWidget);
      expect(find.text('Some place'), findsOneWidget);
      expect(find.textContaining('63.400000'), findsOneWidget);
      expect(find.textContaining('10.400000'), findsOneWidget);
    });

    testWidgets('close icon pops the sheet without invoking any action',
        (tester) async {
      final repo = await _openSheet(tester);
      await tester.tap(find.byIcon(Icons.close));
      await tester.pumpAndSettle();
      expect(find.byType(MarkerInfoSheet), findsNothing);
      expect(repo.deleteCallCount, 0);
    });
  });

  group('MarkerInfoSheet delete flow', () {
    // Delete now lives in the "More" overflow menu (the inline row keeps
    // Navigate / Edit / Export). Open it before reaching for Delete.
    Future<void> openMoreMenu(WidgetTester tester) async {
      await tester.tap(find.text('More'));
      await tester.pumpAndSettle();
    }

    testWidgets('Delete action opens the destructive confirmation dialog',
        (tester) async {
      await _openSheet(tester);

      await openMoreMenu(tester);
      await tester.tap(find.text('Delete'));
      await tester.pumpAndSettle();

      // Dialog appears with the destructive button styled red and a Cancel.
      expect(find.widgetWithText(FilledButton, 'Delete'), findsOneWidget);
      expect(find.widgetWithText(TextButton, 'Cancel'), findsOneWidget);
    });

    testWidgets('confirming the dialog calls deleteMarker exactly once and '
        'pops the sheet', (tester) async {
      final repo = await _openSheet(tester);

      await openMoreMenu(tester);
      await tester.tap(find.text('Delete'));
      await tester.pumpAndSettle();
      await tester.tap(find.widgetWithText(FilledButton, 'Delete'));
      await tester.pumpAndSettle();

      expect(repo.deleteCallCount, 1);
      expect(repo.lastDeletedUuid, 'm1');
      expect(find.byType(MarkerInfoSheet), findsNothing,
          reason: 'sheet must close after a successful delete');
    });

    testWidgets('cancelling the dialog leaves the sheet open and does not '
        'invoke deleteMarker', (tester) async {
      final repo = await _openSheet(tester);

      await openMoreMenu(tester);
      await tester.tap(find.text('Delete'));
      await tester.pumpAndSettle();
      await tester.tap(find.widgetWithText(TextButton, 'Cancel'));
      await tester.pumpAndSettle();

      expect(repo.deleteCallCount, 0);
      expect(find.byType(MarkerInfoSheet), findsOneWidget);
    });
  });

  group('MarkerInfoSheet Navigate Here action', () {
    testWidgets('renders the inline trio plus a More overflow button',
        (tester) async {
      await _openSheet(tester);

      // Navigate is the lead inline action; the long tail folds into "More".
      expect(find.text('Navigate'), findsOneWidget);
      expect(find.text('More'), findsOneWidget);
      expect(find.byIcon(Icons.navigation_outlined), findsOneWidget);

      // The long tail (incl. Delete) is tucked away until the menu opens.
      expect(find.text('Delete'), findsNothing);

      await tester.tap(find.text('More'));
      await tester.pumpAndSettle();
      expect(find.text('Delete'), findsOneWidget);
    });

    testWidgets('tapping Navigate Here starts navigation, closes the sheet, '
        'and seeds the target', (tester) async {
      final container = await _openSheetWith(tester);

      // Initially inactive.
      expect(container.read(navigationStateProvider).isActive, isFalse);

      await tester.tap(find.text('Navigate'));
      await tester.pumpAndSettle();

      final navState = container.read(navigationStateProvider);
      expect(navState.isActive, isTrue);
      expect(navState.target, const LatLng(63.4, 10.4));
      expect(find.byType(MarkerInfoSheet), findsNothing,
          reason: 'sheet should close after starting navigation');
    });

    testWidgets('tapping Navigate Here while already navigating to the same '
        'target is a no-op with a snackbar', (tester) async {
      final container = await _openSheetWith(tester);

      // Pre-seed: already navigating to the same coordinates.
      container
          .read(navigationStateProvider.notifier)
          .startNavigation(const LatLng(63.4, 10.4));
      await tester.pumpAndSettle();

      await tester.tap(find.text('Navigate'));
      await tester.pumpAndSettle();

      // Sheet stays open.
      expect(find.byType(MarkerInfoSheet), findsOneWidget);
      // Snackbar with the friendly hint shows.
      expect(find.text('Already navigating here'), findsOneWidget);
    });

    testWidgets('tapping Navigate Here while navigating elsewhere prompts a '
        'confirm dialog; confirm replaces the target', (tester) async {
      final container = await _openSheetWith(tester);

      container
          .read(navigationStateProvider.notifier)
          .startNavigation(const LatLng(59.9, 10.7));
      await tester.pumpAndSettle();

      await tester.tap(find.text('Navigate'));
      await tester.pumpAndSettle();

      // Dialog appears.
      expect(find.text('Replace navigation target?'), findsOneWidget);

      await tester.tap(find.widgetWithText(FilledButton, 'Replace'));
      await tester.pumpAndSettle();

      final navState = container.read(navigationStateProvider);
      expect(navState.target, const LatLng(63.4, 10.4),
          reason: 'target should be replaced with the marker position');
      expect(find.byType(MarkerInfoSheet), findsNothing);
    });

    testWidgets('cancelling the replace-target dialog leaves the original '
        'target untouched', (tester) async {
      final container = await _openSheetWith(tester);

      container
          .read(navigationStateProvider.notifier)
          .startNavigation(const LatLng(59.9, 10.7));
      await tester.pumpAndSettle();

      await tester.tap(find.text('Navigate'));
      await tester.pumpAndSettle();

      await tester.tap(find.widgetWithText(TextButton, 'Cancel'));
      await tester.pumpAndSettle();

      final navState = container.read(navigationStateProvider);
      expect(navState.target, const LatLng(59.9, 10.7),
          reason: 'cancel leaves the original target in place');
      expect(find.byType(MarkerInfoSheet), findsOneWidget);
    });
  });
}
