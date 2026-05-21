import 'package:flutter/material.dart';
import 'package:flutter_localizations/flutter_localizations.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:latlong2/latlong.dart';
import 'package:turbo/app/l10n/app_localizations.dart';
import 'package:turbo/features/markers/api.dart';
import 'package:turbo/features/sharing/widgets/shared_marker_preview_sheet.dart';

class _FakeRepo extends LocationRepository {
  final List<Marker> added = [];

  @override
  AsyncValue<List<Marker>> build() => const AsyncData([]);

  @override
  Future<void> addMarker(Marker marker) async {
    added.add(marker);
  }
}

Marker _marker({
  String title = 'Cabin',
  String? description = 'A nice spot',
}) =>
    Marker(
      uuid: 'shared-uuid',
      title: title,
      description: description,
      icon: 'home',
      position: const LatLng(60.5, 10.5),
    );

Future<_FakeRepo> _pump(WidgetTester tester, {Marker? marker}) async {
  final repo = _FakeRepo();
  await tester.pumpWidget(
    ProviderScope(
      overrides: [
        locationRepositoryProvider.overrideWith(() => repo),
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
          body: SharedMarkerPreviewSheet(marker: marker ?? _marker()),
        ),
      ),
    ),
  );
  await tester.pumpAndSettle();
  return repo;
}

void main() {
  testWidgets('renders the title and description of the shared marker',
      (tester) async {
    await _pump(tester);
    expect(find.text('Cabin'), findsOneWidget);
    expect(find.text('A nice spot'), findsOneWidget);
  });

  testWidgets('renders coordinates', (tester) async {
    await _pump(tester);
    expect(find.textContaining('60.5'), findsWidgets);
    expect(find.textContaining('10.5'), findsWidgets);
  });

  testWidgets('Save action calls addMarker on the repository',
      (tester) async {
    final repo = await _pump(tester);

    await tester.tap(find.text('Save to my markers'));
    await tester.pumpAndSettle();

    expect(repo.added, hasLength(1));
    expect(repo.added.single.title, 'Cabin');
    expect(repo.added.single.uuid, isNot('shared-uuid'),
        reason:
            'Imported marker should get a fresh local UUID to avoid collisions');
  });
}
