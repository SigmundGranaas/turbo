import 'package:flutter/material.dart';
import 'package:flutter_localizations/flutter_localizations.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:latlong2/latlong.dart';
import 'package:turbo/app/l10n/app_localizations.dart';
import 'package:turbo/core/sharing/api.dart';
import 'package:turbo/features/markers/api.dart';
import 'package:turbo/features/saved_paths/api.dart';
import 'package:turbo/features/sharing/api.dart';

class _NoopMarkerRepo extends LocationRepository {
  @override
  AsyncValue<List<Marker>> build() => const AsyncData([]);
  @override
  Future<void> addMarker(Marker marker) async {}
}

class _NoopPathRepo extends SavedPathRepository {
  @override
  AsyncValue<List<SavedPath>> build() => const AsyncData([]);
  @override
  Future<void> addPath(SavedPath path) async {}
}

Widget _scaffold(Widget child, ProviderContainer container) {
  return UncontrolledProviderScope(
    container: container,
    child: MaterialApp(
      localizationsDelegates: const [
        AppLocalizations.delegate,
        GlobalMaterialLocalizations.delegate,
        GlobalWidgetsLocalizations.delegate,
        GlobalCupertinoLocalizations.delegate,
      ],
      supportedLocales: AppLocalizations.supportedLocales,
      home: SharedPayloadListener(
        child: Scaffold(body: child),
      ),
    ),
  );
}

ProviderContainer _container() {
  final c = ProviderContainer(overrides: [
    locationRepositoryProvider.overrideWith(_NoopMarkerRepo.new),
    savedPathRepositoryProvider.overrideWith(_NoopPathRepo.new),
  ]);
  addTearDown(c.dispose);
  return c;
}

void main() {
  testWidgets(
      'opens marker preview when a marker payload was queued before mount',
      (tester) async {
    final container = _container();
    container.read(pendingShareProvider.notifier).push(
          SharedMarkerPayload(Marker(
            title: 'Hut',
            position: const LatLng(60, 10),
          )),
        );

    await tester.pumpWidget(_scaffold(const Text('home'), container));
    await tester.pumpAndSettle();

    expect(find.byType(SharedMarkerPreviewSheet), findsOneWidget);
    expect(find.text('Hut'), findsOneWidget);
  });

  testWidgets('opens path preview when a path payload arrives after mount',
      (tester) async {
    final container = _container();
    await tester.pumpWidget(_scaffold(const Text('home'), container));
    await tester.pumpAndSettle();

    container.read(pendingShareProvider.notifier).push(
          SharedPathPayload(SavedPath(
            title: 'Loop',
            points: const [LatLng(60, 10), LatLng(60.1, 10.1)],
            distance: 100,
          )),
        );
    await tester.pumpAndSettle();

    expect(find.byType(SharedPathPreviewSheet), findsOneWidget);
    expect(find.text('Loop'), findsOneWidget);
  });

  testWidgets('clears the pending payload after consuming it',
      (tester) async {
    final container = _container();
    container.read(pendingShareProvider.notifier).push(
          SharedMarkerPayload(Marker(
            title: 'X',
            position: const LatLng(60, 10),
          )),
        );

    await tester.pumpWidget(_scaffold(const Text('home'), container));
    await tester.pumpAndSettle();

    expect(container.read(pendingShareProvider), isNull);
  });
}
