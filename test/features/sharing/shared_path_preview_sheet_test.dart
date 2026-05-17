import 'package:flutter/material.dart';
import 'package:flutter_localizations/flutter_localizations.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:latlong2/latlong.dart';
import 'package:turbo/app/l10n/app_localizations.dart';
import 'package:turbo/features/saved_paths/api.dart';
import 'package:turbo/features/sharing/widgets/shared_path_preview_sheet.dart';

class _FakeRepo extends SavedPathRepository {
  final List<SavedPath> added = [];

  @override
  AsyncValue<List<SavedPath>> build() => const AsyncData([]);

  @override
  Future<void> addPath(SavedPath path) async {
    added.add(path);
  }
}

SavedPath _path() => SavedPath(
      uuid: 'shared-uuid',
      title: 'Loop',
      description: 'Round trip',
      points: const [
        LatLng(60.0, 10.0),
        LatLng(60.1, 10.1),
        LatLng(60.2, 10.2),
      ],
      distance: 25000,
    );

Future<_FakeRepo> _pump(WidgetTester tester) async {
  final repo = _FakeRepo();
  await tester.pumpWidget(
    ProviderScope(
      overrides: [
        savedPathRepositoryProvider.overrideWith(() => repo),
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
          body: SharedPathPreviewSheet(path: _path()),
        ),
      ),
    ),
  );
  await tester.pumpAndSettle();
  return repo;
}

void main() {
  testWidgets('renders title, description and point count', (tester) async {
    await _pump(tester);
    expect(find.text('Loop'), findsOneWidget);
    expect(find.text('Round trip'), findsOneWidget);
    expect(find.textContaining('3'), findsWidgets); // point count
  });

  testWidgets('Save action calls addPath on the repository', (tester) async {
    final repo = await _pump(tester);

    await tester.tap(find.text('Save to my routes'));
    await tester.pumpAndSettle();

    expect(repo.added, hasLength(1));
    expect(repo.added.single.title, 'Loop');
    expect(repo.added.single.uuid, isNot('shared-uuid'),
        reason: 'Imported path should get a fresh local UUID');
  });
}
