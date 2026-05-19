import 'package:flutter/material.dart';
import 'package:flutter_localizations/flutter_localizations.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:latlong2/latlong.dart';
import 'package:shared_preferences/shared_preferences.dart';
import 'package:turbo/app/l10n/app_localizations.dart';
import 'package:turbo/features/saved_paths/api.dart';

/// Captures deletePath calls so the "Undo" assertion doesn't need a real DB.
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

SavedPath _path({String title = 'Morning walk'}) => SavedPath(
      title: title,
      points: const [LatLng(59.9, 10.7), LatLng(60.0, 10.8)],
      distance: 1234,
    );

Widget _harness({
  required SavedPath savedPath,
  required _FakeRepo repo,
}) {
  return ProviderScope(
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
        body: Consumer(
          builder: (context, ref, _) => Center(
            child: ElevatedButton(
              onPressed: () =>
                  showPathSavedFeedback(context, ref, savedPath),
              child: const Text('FIRE'),
            ),
          ),
        ),
      ),
    ),
  );
}

void main() {
  setUp(() {
    SharedPreferences.setMockInitialValues({});
  });

  group('showPathSavedFeedback', () {
    testWidgets('shows a snackbar that names the saved path', (tester) async {
      final repo = _FakeRepo();
      await tester.pumpWidget(_harness(
        savedPath: _path(title: 'Morning walk'),
        repo: repo,
      ));
      await tester.pump();

      await tester.tap(find.text('FIRE'));
      await tester.pump();

      expect(find.text('Saved "Morning walk"'), findsOneWidget);
      expect(find.text('Undo'), findsOneWidget);

      // Dismiss the snackbar so the framework's teardown isn't fighting a
      // long-lived animation.
      await tester.pump(const Duration(seconds: 10));
    });

    testWidgets('forces saved-paths visibility ON even if it was OFF',
        (tester) async {
      SharedPreferences.setMockInitialValues({'savedPathsVisible': false});
      final repo = _FakeRepo();

      await tester.pumpWidget(_harness(
        savedPath: _path(),
        repo: repo,
      ));
      await tester.pump();

      // Read the visibility notifier from the live container. Use the FIRE
      // button's context (a descendant of ProviderScope) — containerOf walks
      // UP the tree to find the InheritedWidget, so passing ProviderScope's
      // own element wouldn't find it.
      final element = tester.element(find.text('FIRE'));
      final container = ProviderScope.containerOf(element);
      container.listen(savedPathsVisibleProvider, (_, _) {});
      // Two pumps flush the async _loadFromPrefs call inside the notifier.
      await tester.pump();
      await tester.pump();
      expect(container.read(savedPathsVisibleProvider), isFalse,
          reason: 'pre-condition: the layer is off');

      await tester.tap(find.text('FIRE'));
      await tester.pump();

      expect(container.read(savedPathsVisibleProvider), isTrue,
          reason: 'helper should have forced visibility on');

      await tester.pump(const Duration(seconds: 10));
    });

    testWidgets('Undo deletes the saved path from the repository',
        (tester) async {
      final repo = _FakeRepo();
      final saved = _path(title: 'Trip to undo');

      await tester.pumpWidget(_harness(savedPath: saved, repo: repo));
      await tester.pump();

      await tester.tap(find.text('FIRE'));
      await tester.pump();
      // Snackbar entrance animation is ~250 ms; until it finishes the action
      // sits inside an IgnorePointer and taps don't reach it.
      await tester.pump(const Duration(milliseconds: 750));
      await tester.tap(find.text('Undo'));
      // Drain the onPressed microtask + the snackbar exit animation.
      await tester.pump(const Duration(milliseconds: 500));

      expect(repo.deleteCallCount, 1);
      expect(repo.lastDeletedUuid, saved.uuid);

      await tester.pump(const Duration(seconds: 10));
    });
  });
}
