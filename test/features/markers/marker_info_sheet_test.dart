import 'package:flutter/material.dart';
import 'package:flutter_localizations/flutter_localizations.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:latlong2/latlong.dart';
import 'package:turbo/features/collections/api.dart';
import 'package:turbo/features/markers/api.dart';
import 'package:turbo/app/l10n/app_localizations.dart';

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

Marker _marker() => Marker(
      uuid: 'm1',
      title: 'My Pin',
      description: 'Some place',
      position: const LatLng(63.4, 10.4),
    );

Future<_FakeRepo> _openSheet(WidgetTester tester, {Marker? marker}) async {
  final repo = _FakeRepo();
  final m = marker ?? _marker();
  await tester.pumpWidget(
    ProviderScope(
      overrides: [
        locationRepositoryProvider.overrideWith(() => repo),
        collectionRepositoryProvider.overrideWith(() => _FakeCollectionRepo()),
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
                onPressed: () => showModalBottomSheet<MarkerInfoResult>(
                  context: ctx,
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
  return repo;
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
    testWidgets('Delete action opens the destructive confirmation dialog',
        (tester) async {
      await _openSheet(tester);

      await tester.tap(find.text('Delete'));
      await tester.pumpAndSettle();

      // Dialog appears with the destructive button styled red and a Cancel.
      expect(find.widgetWithText(FilledButton, 'Delete'), findsOneWidget);
      expect(find.widgetWithText(TextButton, 'Cancel'), findsOneWidget);
    });

    testWidgets('confirming the dialog calls deleteMarker exactly once and '
        'pops the sheet', (tester) async {
      final repo = await _openSheet(tester);

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

      await tester.tap(find.text('Delete'));
      await tester.pumpAndSettle();
      await tester.tap(find.widgetWithText(TextButton, 'Cancel'));
      await tester.pumpAndSettle();

      expect(repo.deleteCallCount, 0);
      expect(find.byType(MarkerInfoSheet), findsOneWidget);
    });
  });
}
