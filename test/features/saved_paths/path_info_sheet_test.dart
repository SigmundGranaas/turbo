import 'package:flutter/material.dart';
import 'package:flutter_localizations/flutter_localizations.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:latlong2/latlong.dart';
import 'package:turbo/features/saved_paths/api.dart';
import 'package:turbo/l10n/app_localizations.dart';

class _FakeRepo extends SavedPathRepository {
  int deleteCallCount = 0;
  String? lastDeletedUuid;

  @override
  AsyncValue<List<SavedPath>> build() => const AsyncData([]);

  @override
  Future<void> deletePath(String uuid) async {
    deleteCallCount++;
    lastDeletedUuid = uuid;
  }
}

SavedPath _path() => SavedPath(
      uuid: 'p1',
      title: 'Morning Run',
      description: 'Down the river',
      points: const [LatLng(63.4, 10.4), LatLng(63.5, 10.5)],
      distance: 1234.5,
      createdAt: DateTime(2025, 5, 1),
    );

Future<_FakeRepo> _openSheet(WidgetTester tester) async {
  final repo = _FakeRepo();
  final p = _path();
  await tester.pumpWidget(
    ProviderScope(
      overrides: [savedPathRepositoryProvider.overrideWith(() => repo)],
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
                  builder: (_) => PathInfoSheet(path: p),
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
  group('PathInfoSheet header', () {
    testWidgets('renders title, distance, and description', (tester) async {
      await _openSheet(tester);
      expect(find.text('Morning Run'), findsOneWidget);
      expect(find.text('Down the river'), findsOneWidget);
      // Distance shown in km with 2-decimal precision: 1234.5 m → 1.23 km
      expect(find.textContaining('1.23 km'), findsOneWidget);
    });
  });

  group('PathInfoSheet delete flow', () {
    testWidgets('Delete opens destructive dialog', (tester) async {
      await _openSheet(tester);
      await tester.tap(find.text('Delete'));
      await tester.pumpAndSettle();
      expect(find.widgetWithText(FilledButton, 'Delete'), findsOneWidget);
      expect(find.widgetWithText(TextButton, 'Cancel'), findsOneWidget);
    });

    testWidgets('confirming deletes the path once and closes the sheet',
        (tester) async {
      final repo = await _openSheet(tester);
      await tester.tap(find.text('Delete'));
      await tester.pumpAndSettle();
      await tester.tap(find.widgetWithText(FilledButton, 'Delete'));
      await tester.pumpAndSettle();

      expect(repo.deleteCallCount, 1);
      expect(repo.lastDeletedUuid, 'p1');
      expect(find.byType(PathInfoSheet), findsNothing);
    });

    testWidgets('cancelling keeps the path and leaves the sheet open',
        (tester) async {
      final repo = await _openSheet(tester);
      await tester.tap(find.text('Delete'));
      await tester.pumpAndSettle();
      await tester.tap(find.widgetWithText(TextButton, 'Cancel'));
      await tester.pumpAndSettle();

      expect(repo.deleteCallCount, 0);
      expect(find.byType(PathInfoSheet), findsOneWidget);
    });
  });
}
