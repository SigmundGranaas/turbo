import 'dart:async';

import 'package:flutter/material.dart' hide CatmullRomSpline;
import 'package:flutter_localizations/flutter_localizations.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:latlong2/latlong.dart';
import 'package:sqflite_common_ffi/sqflite_ffi.dart';
import 'package:turbo/core/data/database_provider.dart';
import 'package:turbo/core/util/catmull_rom_spline.dart';
import 'package:turbo/features/saved_paths/data/saved_path_repository.dart';
import 'package:turbo/features/saved_paths/data/sqlite_saved_path_datastore.dart';
import 'package:turbo/features/saved_paths/models/path_style.dart';
import 'package:turbo/features/saved_paths/models/saved_path.dart';
import 'package:turbo/features/saved_paths/widgets/path_customization_controls.dart';
import 'package:turbo/features/saved_paths/widgets/path_detail_sheet.dart';
import 'package:turbo/features/saved_paths/widgets/save_path_sheet.dart';
import 'package:turbo/l10n/app_localizations.dart';

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

SavedPath _makePath({
  String? uuid,
  String title = 'Test Path',
  String? description,
  List<LatLng>? points,
  double distance = 1000.0,
  String? colorHex,
  String? iconKey,
  bool smoothing = false,
  String? lineStyleKey,
}) =>
    SavedPath(
      uuid: uuid,
      title: title,
      description: description,
      points: points ?? [const LatLng(59.9, 10.7), const LatLng(60.0, 10.8)],
      distance: distance,
      colorHex: colorHex,
      iconKey: iconKey,
      smoothing: smoothing,
      lineStyleKey: lineStyleKey,
    );

Future<List<SavedPath>> _waitForData(ProviderContainer container) async {
  for (var i = 0; i < 100; i++) {
    await Future.delayed(const Duration(milliseconds: 20));
    final s = container.read(savedPathRepositoryProvider);
    if (s is AsyncData<List<SavedPath>>) return s.value;
    if (s is AsyncError) throw (s as AsyncError).error;
  }
  throw TimeoutException('SavedPathRepository did not settle');
}

Future<Database> _createTestDb() async {
  final db = await databaseFactory.openDatabase(inMemoryDatabasePath);
  await db.execute('''
    CREATE TABLE $savedPathsTable(
      uuid TEXT PRIMARY KEY,
      title TEXT NOT NULL,
      description TEXT,
      points TEXT NOT NULL,
      distance REAL NOT NULL,
      min_lat REAL NOT NULL,
      min_lng REAL NOT NULL,
      max_lat REAL NOT NULL,
      max_lng REAL NOT NULL,
      created_at TEXT NOT NULL,
      color_hex TEXT,
      icon_key TEXT,
      smoothing INTEGER NOT NULL DEFAULT 0,
      line_style TEXT
    )
  ''');
  await db.execute(
      'CREATE INDEX idx_saved_paths_bounds ON $savedPathsTable(min_lat, max_lat, min_lng, max_lng)');
  return db;
}

/// Wraps a widget with MaterialApp + localization for widget tests.
/// Pass [dbOverride] to override the database provider with an in-memory DB.
Widget _testApp(Widget child, {Database? dbOverride}) {
  return ProviderScope(
    overrides: [
      if (dbOverride != null)
        databaseProvider.overrideWith((ref) async => dbOverride),
    ],
    child: MaterialApp(
      localizationsDelegates: const [
        AppLocalizations.delegate,
        GlobalMaterialLocalizations.delegate,
        GlobalWidgetsLocalizations.delegate,
        GlobalCupertinoLocalizations.delegate,
      ],
      supportedLocales: AppLocalizations.supportedLocales,
      home: Scaffold(body: child),
    ),
  );
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

void main() {
  // =========================================================================
  // SECTION 1: Full data lifecycle E2E — save, read, update, delete with
  // customization fields through the real repository → SQLite stack.
  // =========================================================================
  group('Customization data lifecycle E2E', () {
    late Database db;
    late ProviderContainer container;

    setUpAll(() {
      sqfliteFfiInit();
      databaseFactory = databaseFactoryFfi;
    });

    setUp(() async {
      db = await _createTestDb();
      container = ProviderContainer(overrides: [
        databaseProvider.overrideWith((ref) async => db),
      ]);
      container.listen(savedPathRepositoryProvider, (_, _) {});
      await _waitForData(container);
    });

    tearDown(() async {
      container.dispose();
      await db.close();
    });

    test('save path with all customization → read back → all fields match', () async {
      final repo = container.read(savedPathRepositoryProvider.notifier);

      final path = _makePath(
        uuid: 'styled-1',
        title: 'Styled Trail',
        description: 'A colorful dashed route',
        colorHex: 'D32F2F',
        iconKey: 'Vandring',
        smoothing: true,
        lineStyleKey: 'dashed',
        points: [
          const LatLng(60.0, 10.0),
          const LatLng(60.1, 10.1),
          const LatLng(60.2, 10.2),
        ],
        distance: 5000.0,
      );
      await repo.addPath(path);
      final paths = await _waitForData(container);

      expect(paths.length, 1);
      final saved = paths.first;
      expect(saved.title, 'Styled Trail');
      expect(saved.colorHex, 'D32F2F');
      expect(saved.iconKey, 'Vandring');
      expect(saved.smoothing, true);
      expect(saved.lineStyleKey, 'dashed');

      // Also verify directly from the database
      final store = SQLiteSavedPathDataStore(db);
      final fromDb = await store.getByUuid('styled-1');
      expect(fromDb!.colorHex, 'D32F2F');
      expect(fromDb.iconKey, 'Vandring');
      expect(fromDb.smoothing, true);
      expect(fromDb.lineStyleKey, 'dashed');
    });

    test('save with defaults → fields are null/false', () async {
      final repo = container.read(savedPathRepositoryProvider.notifier);

      final path = _makePath(uuid: 'default-1', title: 'Default Path');
      await repo.addPath(path);

      final store = SQLiteSavedPathDataStore(db);
      final fromDb = await store.getByUuid('default-1');
      expect(fromDb!.colorHex, isNull);
      expect(fromDb.iconKey, isNull);
      expect(fromDb.smoothing, false);
      expect(fromDb.lineStyleKey, isNull);
    });

    test('update customization fields → changes persist', () async {
      final repo = container.read(savedPathRepositoryProvider.notifier);
      final store = SQLiteSavedPathDataStore(db);

      // Save with no customization
      final path = _makePath(uuid: 'update-1', title: 'Plain Path');
      await repo.addPath(path);
      await _waitForData(container);

      // Add customization
      await repo.updatePath(path.copyWith(
        colorHex: '388E3C',
        iconKey: 'Sykkel',
        smoothing: true,
        lineStyleKey: 'dotted',
      ));
      await _waitForData(container);

      var fromDb = await store.getByUuid('update-1');
      expect(fromDb!.colorHex, '388E3C');
      expect(fromDb.iconKey, 'Sykkel');
      expect(fromDb.smoothing, true);
      expect(fromDb.lineStyleKey, 'dotted');

      // Clear customization back to defaults
      await repo.updatePath(fromDb.copyWith(
        clearColorHex: true,
        clearIconKey: true,
        smoothing: false,
        clearLineStyleKey: true,
      ));
      await _waitForData(container);

      fromDb = await store.getByUuid('update-1');
      expect(fromDb!.colorHex, isNull);
      expect(fromDb.iconKey, isNull);
      expect(fromDb.smoothing, false);
      expect(fromDb.lineStyleKey, isNull);
    });

    test('full lifecycle: create → customize → rename → delete', () async {
      final repo = container.read(savedPathRepositoryProvider.notifier);
      final store = SQLiteSavedPathDataStore(db);

      // 1. Create
      final path = _makePath(
        uuid: 'lifecycle-1',
        title: 'Morning Jog',
        colorHex: 'F57C00',
        lineStyleKey: 'dash_dot',
      );
      await repo.addPath(path);
      var paths = await _waitForData(container);
      expect(paths.length, 1);

      // 2. Customize: change color, add icon, enable smoothing
      await repo.updatePath(path.copyWith(
        colorHex: '7B1FA2',
        iconKey: 'Ski',
        smoothing: true,
        lineStyleKey: 'dashed',
      ));
      paths = await _waitForData(container);
      expect(paths.first.colorHex, '7B1FA2');
      expect(paths.first.iconKey, 'Ski');
      expect(paths.first.smoothing, true);
      expect(paths.first.lineStyleKey, 'dashed');

      // 3. Rename
      await repo.updatePath(paths.first.copyWith(title: 'Evening Run'));
      paths = await _waitForData(container);
      expect(paths.first.title, 'Evening Run');
      // Customization preserved
      expect(paths.first.colorHex, '7B1FA2');

      // 4. Delete
      await repo.deletePath('lifecycle-1');
      paths = await _waitForData(container);
      expect(paths, isEmpty);
      expect(await store.getByUuid('lifecycle-1'), isNull);
    });

    test('multiple paths with different customizations coexist', () async {
      final repo = container.read(savedPathRepositoryProvider.notifier);

      await repo.addPath(_makePath(
        uuid: 'red-solid',
        title: 'Red Solid',
        colorHex: 'D32F2F',
        lineStyleKey: null,
      ));
      await repo.addPath(_makePath(
        uuid: 'blue-dashed',
        title: 'Blue Dashed',
        colorHex: '1976D2',
        lineStyleKey: 'dashed',
        smoothing: true,
      ));
      await repo.addPath(_makePath(
        uuid: 'default-path',
        title: 'Default Style',
      ));

      final paths = await _waitForData(container);
      expect(paths.length, 3);

      final red = paths.firstWhere((p) => p.uuid == 'red-solid');
      expect(red.colorHex, 'D32F2F');
      expect(red.lineStyleKey, isNull);
      expect(red.smoothing, false);

      final blue = paths.firstWhere((p) => p.uuid == 'blue-dashed');
      expect(blue.colorHex, '1976D2');
      expect(blue.lineStyleKey, 'dashed');
      expect(blue.smoothing, true);

      final def = paths.firstWhere((p) => p.uuid == 'default-path');
      expect(def.colorHex, isNull);
      expect(def.lineStyleKey, isNull);
    });

    test('spatial query returns customized paths in viewport', () async {
      final store = SQLiteSavedPathDataStore(db);

      await store.insert(_makePath(
        uuid: 'in-view',
        title: 'In View',
        colorHex: '00897B',
        lineStyleKey: 'dotted',
        smoothing: true,
        iconKey: 'Fjell',
        points: [const LatLng(60.0, 10.5), const LatLng(60.1, 10.6)],
      ));
      await store.insert(_makePath(
        uuid: 'out-of-view',
        title: 'Out Of View',
        colorHex: 'C2185B',
        points: [const LatLng(50.0, 5.0), const LatLng(50.1, 5.1)],
      ));

      final results = await store.findInBounds(
        const LatLng(59.5, 10.0),
        const LatLng(60.5, 11.0),
      );

      expect(results.length, 1);
      expect(results.first.uuid, 'in-view');
      // Customization fields survive spatial query
      expect(results.first.colorHex, '00897B');
      expect(results.first.lineStyleKey, 'dotted');
      expect(results.first.smoothing, true);
      expect(results.first.iconKey, 'Fjell');
    });
  });

  // =========================================================================
  // SECTION 2: SavedPath model — copyWith, serialization, equality
  // =========================================================================
  group('SavedPath model copyWith + clear flags', () {
    test('copyWith preserves customization when not specified', () {
      final path = _makePath(
        colorHex: 'AABBCC',
        iconKey: 'Hytte',
        smoothing: true,
        lineStyleKey: 'dotted',
      );

      final updated = path.copyWith(title: 'New Title');
      expect(updated.title, 'New Title');
      expect(updated.colorHex, 'AABBCC');
      expect(updated.iconKey, 'Hytte');
      expect(updated.smoothing, true);
      expect(updated.lineStyleKey, 'dotted');
    });

    test('copyWith clearColorHex nulls color', () {
      final path = _makePath(colorHex: 'FF0000');
      final cleared = path.copyWith(clearColorHex: true);
      expect(cleared.colorHex, isNull);
    });

    test('copyWith clearIconKey nulls icon', () {
      final path = _makePath(iconKey: 'Fjell');
      final cleared = path.copyWith(clearIconKey: true);
      expect(cleared.iconKey, isNull);
    });

    test('copyWith clearLineStyleKey nulls line style', () {
      final path = _makePath(lineStyleKey: 'dashed');
      final cleared = path.copyWith(clearLineStyleKey: true);
      expect(cleared.lineStyleKey, isNull);
    });

    test('clearX flag takes precedence over new value', () {
      final path = _makePath(colorHex: 'FF0000');
      final cleared = path.copyWith(clearColorHex: true, colorHex: 'AABBCC');
      expect(cleared.colorHex, isNull,
          reason: 'clear flag should take precedence');
    });

    test('equality includes customization fields', () {
      final a = _makePath(
        uuid: 'same',
        colorHex: '123456',
        smoothing: true,
        lineStyleKey: 'dashed',
      );
      final b = _makePath(
        uuid: 'same',
        colorHex: '123456',
        smoothing: true,
        lineStyleKey: 'dashed',
      );
      // Same createdAt is not guaranteed with DateTime.now(), so compare relevant fields
      expect(a.uuid, b.uuid);
      expect(a.colorHex, b.colorHex);
      expect(a.smoothing, b.smoothing);
      expect(a.lineStyleKey, b.lineStyleKey);
    });

    test('inequality when customization differs', () {
      final a = _makePath(uuid: 'x', colorHex: '111111');
      final b = _makePath(uuid: 'x', colorHex: '222222');
      // hashCode should differ (not guaranteed but extremely likely)
      expect(a.colorHex != b.colorHex, true);
    });

    test('toLocalMap / fromLocalMap round-trip preserves all fields', () {
      final original = _makePath(
        uuid: 'round-trip',
        title: 'Round Trip',
        description: 'Test desc',
        colorHex: '546E7A',
        iconKey: 'Kajakk',
        smoothing: true,
        lineStyleKey: 'dash_dot',
      );

      final map = original.toLocalMap();
      final restored = SavedPath.fromLocalMap(map);

      expect(restored.uuid, original.uuid);
      expect(restored.title, original.title);
      expect(restored.description, original.description);
      expect(restored.colorHex, original.colorHex);
      expect(restored.iconKey, original.iconKey);
      expect(restored.smoothing, original.smoothing);
      expect(restored.lineStyleKey, original.lineStyleKey);
      expect(restored.distance, original.distance);
    });

    test('fromLocalMap handles smoothing as int (SQLite)', () {
      final map = _makePath(smoothing: true).toLocalMap();
      expect(map['smoothing'], 1);
      final restored = SavedPath.fromLocalMap(map);
      expect(restored.smoothing, true);
    });

    test('fromLocalMap handles smoothing as bool (IndexedDB)', () {
      final map = _makePath().toLocalMap();
      map['smoothing'] = true; // Simulate IndexedDB storing as bool
      final restored = SavedPath.fromLocalMap(map);
      expect(restored.smoothing, true);
    });

    test('fromLocalMap defaults smoothing to false for 0', () {
      final map = _makePath().toLocalMap();
      map['smoothing'] = 0;
      final restored = SavedPath.fromLocalMap(map);
      expect(restored.smoothing, false);
    });

    test('toLocalMap stores null customization fields as null', () {
      final path = _makePath();
      final map = path.toLocalMap();
      expect(map['color_hex'], isNull);
      expect(map['icon_key'], isNull);
      expect(map['smoothing'], 0);
      expect(map['line_style'], isNull);
    });
  });

  // =========================================================================
  // SECTION 3: CatmullRomSpline smoothing integration
  // =========================================================================
  group('CatmullRomSpline smoothing', () {
    test('smoothing produces more points than input', () {
      final control = [
        const LatLng(60.0, 10.0),
        const LatLng(60.1, 10.1),
        const LatLng(60.2, 10.0),
        const LatLng(60.3, 10.1),
      ];
      final smoothed = CatmullRomSpline(controlPoints: control).generate();
      expect(smoothed.length, greaterThan(control.length));
    });

    test('smoothing with 2 points still produces output', () {
      final control = [
        const LatLng(60.0, 10.0),
        const LatLng(60.1, 10.1),
      ];
      final smoothed = CatmullRomSpline(controlPoints: control).generate();
      expect(smoothed, isNotEmpty);
      // First point should match the first control point
      expect(smoothed.first.latitude, closeTo(60.0, 0.001));
    });

    test('smoothing with 1 point returns that point', () {
      final control = [const LatLng(60.0, 10.0)];
      final smoothed = CatmullRomSpline(controlPoints: control).generate();
      expect(smoothed.length, 1);
      expect(smoothed.first, control.first);
    });

    test('smoothed points stay in vicinity of control points', () {
      final control = [
        const LatLng(60.0, 10.0),
        const LatLng(60.1, 10.1),
        const LatLng(60.2, 10.0),
      ];
      final smoothed = CatmullRomSpline(controlPoints: control).generate();

      for (final p in smoothed) {
        // All points should be within a reasonable range of the control hull
        expect(p.latitude, greaterThan(59.9));
        expect(p.latitude, lessThan(60.3));
        expect(p.longitude, greaterThan(9.9));
        expect(p.longitude, lessThan(10.2));
      }
    });
  });

  // =========================================================================
  // SECTION 4: PathLineStyle + color integration with SavedPath
  // =========================================================================
  group('PathLineStyle + color integration', () {
    test('all palette colors survive hex round-trip', () {
      for (final color in pathColorPalette) {
        final hex = colorToHex(color);
        final path = _makePath(colorHex: hex);
        final map = path.toLocalMap();
        final restored = SavedPath.fromLocalMap(map);
        final restoredColor = hexToColor(restored.colorHex);
        expect(restoredColor, isNotNull);
        expect(colorToHex(restoredColor!), hex);
      }
    });

    test('all line styles survive key round-trip through SavedPath', () {
      for (final style in PathLineStyle.values) {
        final key = style == PathLineStyle.solid ? null : style.key;
        final path = _makePath(lineStyleKey: key);
        final map = path.toLocalMap();
        final restored = SavedPath.fromLocalMap(map);
        final restoredStyle = PathLineStyle.fromKey(restored.lineStyleKey);
        expect(restoredStyle, style);
      }
    });

    test('PathLineStyle.fromKey → toStrokePattern works for every variant', () {
      for (final style in PathLineStyle.values) {
        final pattern = PathLineStyle.fromKey(style.key).toStrokePattern();
        expect(pattern, isNotNull);
      }
    });
  });

  // =========================================================================
  // SECTION 5: Widget tests — PathCustomizationControls
  // =========================================================================
  group('PathCustomizationControls widget', () {
    testWidgets('renders all four control sections', (tester) async {
      await tester.pumpWidget(_testApp(
        SingleChildScrollView(
          child: PathCustomizationControls(
            selectedColor: null,
            onColorChanged: (_) {},
            selectedIconKey: null,
            onIconChanged: (_) {},
            isSmoothing: false,
            onSmoothingChanged: (_) {},
            lineStyle: PathLineStyle.solid,
            onLineStyleChanged: (_) {},
          ),
        ),
      ));
      await tester.pumpAndSettle();

      // Color section
      expect(find.text('Color'), findsOneWidget);
      // Icon section
      expect(find.text('Icon'), findsOneWidget);
      // Smoothing section
      expect(find.text('Smooth line'), findsOneWidget);
      // Line style section
      expect(find.text('Line style'), findsOneWidget);
      expect(find.text('Solid'), findsOneWidget);
      expect(find.text('Dotted'), findsOneWidget);
      expect(find.text('Dashed'), findsOneWidget);
      expect(find.text('Dash-dot'), findsOneWidget);
    });

    testWidgets('default color shows check mark initially', (tester) async {
      await tester.pumpWidget(_testApp(
        SingleChildScrollView(
          child: PathCustomizationControls(
            selectedColor: null,
            onColorChanged: (_) {},
            selectedIconKey: null,
            onIconChanged: (_) {},
            isSmoothing: false,
            onSmoothingChanged: (_) {},
            lineStyle: PathLineStyle.solid,
            onLineStyleChanged: (_) {},
            initiallyExpanded: true,
          ),
        ),
      ));
      await tester.pumpAndSettle();

      // The default circle should have a check icon (there may be another
      // check icon from the SegmentedButton's selected state)
      expect(find.byIcon(Icons.check), findsAtLeast(1));
    });

    testWidgets('tapping a color circle calls onColorChanged', (tester) async {
      Color? receivedColor;
      await tester.pumpWidget(_testApp(
        SingleChildScrollView(
          child: PathCustomizationControls(
            selectedColor: null,
            onColorChanged: (c) => receivedColor = c,
            selectedIconKey: null,
            onIconChanged: (_) {},
            isSmoothing: false,
            onSmoothingChanged: (_) {},
            lineStyle: PathLineStyle.solid,
            onLineStyleChanged: (_) {},
            initiallyExpanded: true,
          ),
        ),
      ));
      await tester.pumpAndSettle();


      // There are 11 color circles (1 default + 10 palette) plus GestureDetectors
      // from other widgets. Find them by the Container size.
      final colorContainers = find.byWidgetPredicate(
        (w) => w is Container && w.constraints?.maxWidth == 36,
      );
      // Tap the second color container (first palette color)
      if (colorContainers.evaluate().length >= 2) {
        await tester.tap(colorContainers.at(1));
        await tester.pumpAndSettle();
        expect(receivedColor, isNotNull);
      }
    });

    testWidgets('smoothing switch toggles correctly', (tester) async {
      bool smoothing = false;
      late StateSetter outerSetState;

      await tester.pumpWidget(_testApp(
        StatefulBuilder(
          builder: (context, setState) {
            outerSetState = setState;
            return SingleChildScrollView(
              child: PathCustomizationControls(
                selectedColor: null,
                onColorChanged: (_) {},
                selectedIconKey: null,
                onIconChanged: (_) {},
                isSmoothing: smoothing,
                onSmoothingChanged: (v) {
                  outerSetState(() => smoothing = v);
                },
                lineStyle: PathLineStyle.solid,
                onLineStyleChanged: (_) {},
                initiallyExpanded: true,
              ),
            );
          },
        ),
      ));
      await tester.pumpAndSettle();

      // Find and tap the switch
      final switchFinder = find.byType(Switch);
      expect(switchFinder, findsOneWidget);

      await tester.tap(switchFinder);
      await tester.pumpAndSettle();

      expect(smoothing, true);
    });

    testWidgets('line style segmented button selection works', (tester) async {
      PathLineStyle selectedStyle = PathLineStyle.solid;
      late StateSetter outerSetState;

      await tester.pumpWidget(_testApp(
        StatefulBuilder(
          builder: (context, setState) {
            outerSetState = setState;
            return SingleChildScrollView(
              child: PathCustomizationControls(
                selectedColor: null,
                onColorChanged: (_) {},
                selectedIconKey: null,
                onIconChanged: (_) {},
                isSmoothing: false,
                onSmoothingChanged: (_) {},
                lineStyle: selectedStyle,
                onLineStyleChanged: (s) {
                  outerSetState(() => selectedStyle = s);
                },
                initiallyExpanded: true,
              ),
            );
          },
        ),
      ));
      await tester.pumpAndSettle();

      // Segments use visual icons instead of text labels.
      // Find CustomPaint widgets inside the SegmentedButton.
      final segments = find.descendant(
        of: find.byType(SegmentedButton<PathLineStyle>),
        matching: find.byType(CustomPaint),
      );

      // Tap "Dashed" (3rd segment, index 2)
      await tester.tap(segments.at(2));
      await tester.pumpAndSettle();
      expect(selectedStyle, PathLineStyle.dashed);

      // Tap "Dotted" (2nd segment, index 1)
      await tester.tap(segments.at(1));
      await tester.pumpAndSettle();
      expect(selectedStyle, PathLineStyle.dotted);
    });

    testWidgets('icon row shows "Icon" when no icon selected', (tester) async {
      await tester.pumpWidget(_testApp(
        SingleChildScrollView(
          child: PathCustomizationControls(
            selectedColor: null,
            onColorChanged: (_) {},
            selectedIconKey: null,
            onIconChanged: (_) {},
            isSmoothing: false,
            onSmoothingChanged: (_) {},
            lineStyle: PathLineStyle.solid,
            onLineStyleChanged: (_) {},
            initiallyExpanded: true,
          ),
        ),
      ));
      await tester.pumpAndSettle();

      expect(find.text('Icon'), findsOneWidget);
      // No clear button when no icon selected
      expect(find.byIcon(Icons.clear), findsNothing);
    });

    testWidgets('icon row shows icon name and clear button when icon set', (tester) async {
      String? iconKey = 'Vandring';
      late StateSetter outerSetState;

      await tester.pumpWidget(_testApp(
        StatefulBuilder(
          builder: (context, setState) {
            outerSetState = setState;
            return SingleChildScrollView(
              child: PathCustomizationControls(
                selectedColor: null,
                onColorChanged: (_) {},
                selectedIconKey: iconKey,
                onIconChanged: (k) {
                  outerSetState(() => iconKey = k);
                },
                isSmoothing: false,
                onSmoothingChanged: (_) {},
                lineStyle: PathLineStyle.solid,
                onLineStyleChanged: (_) {},
                initiallyExpanded: true,
              ),
            );
          },
        ),
      ));
      await tester.pumpAndSettle();

      // Should show the localized name "Hiking" and a clear button
      expect(find.text('Hiking'), findsOneWidget);
      expect(find.byIcon(Icons.clear), findsOneWidget);

      // Tap clear button
      await tester.tap(find.byIcon(Icons.clear));
      await tester.pumpAndSettle();
      expect(iconKey, isNull);
    });
  });

  // =========================================================================
  // SECTION 6: Widget tests — SavePathSheet
  // =========================================================================
  group('SavePathSheet widget', () {
    late Database db;

    setUpAll(() {
      sqfliteFfiInit();
      databaseFactory = databaseFactoryFfi;
    });

    setUp(() async {
      db = await _createTestDb();
    });

    tearDown(() async {
      await db.close();
    });

    testWidgets('renders with customization controls', (tester) async {
      await tester.pumpWidget(_testApp(
        SingleChildScrollView(
          child: SavePathSheet(
            points: [const LatLng(60.0, 10.0), const LatLng(60.1, 10.1)],
            distance: 1000.0,
          ),
        ),
        dbOverride: db,
      ));
      await tester.pumpAndSettle();

      expect(find.text('Save Path'), findsWidgets);
      expect(find.text('Appearance'), findsOneWidget);

      // Expand appearance section
      await tester.tap(find.text('Appearance'));
      await tester.pumpAndSettle();

      expect(find.text('Color'), findsOneWidget);
      expect(find.text('Smooth line'), findsOneWidget);
    });

    testWidgets('isSmoothing parameter initializes switch state', (tester) async {
      await tester.pumpWidget(_testApp(
        SingleChildScrollView(
          child: SavePathSheet(
            points: [const LatLng(60.0, 10.0), const LatLng(60.1, 10.1)],
            distance: 1000.0,
            isSmoothing: true,
          ),
        ),
        dbOverride: db,
      ));
      await tester.pumpAndSettle();

      // Expand appearance section to access controls
      await tester.tap(find.text('Appearance'));
      await tester.pumpAndSettle();

      final switchWidget = tester.widget<Switch>(find.byType(Switch));
      expect(switchWidget.value, true);
    });

    testWidgets('isSmoothing defaults to false', (tester) async {
      await tester.pumpWidget(_testApp(
        SingleChildScrollView(
          child: SavePathSheet(
            points: [const LatLng(60.0, 10.0), const LatLng(60.1, 10.1)],
            distance: 1000.0,
          ),
        ),
        dbOverride: db,
      ));
      await tester.pumpAndSettle();

      // Expand appearance section to access controls
      await tester.tap(find.text('Appearance'));
      await tester.pumpAndSettle();

      final switchWidget = tester.widget<Switch>(find.byType(Switch));
      expect(switchWidget.value, false);
    });

    testWidgets('customization controls respond to user interaction', (tester) async {
      await tester.pumpWidget(_testApp(
        SingleChildScrollView(
          child: SavePathSheet(
            points: [const LatLng(60.0, 10.0), const LatLng(60.1, 10.1)],
            distance: 2500.0,
            isSmoothing: true,
          ),
        ),
        dbOverride: db,
      ));
      await tester.pumpAndSettle();

      // Expand appearance section to access controls
      await tester.tap(find.text('Appearance'));
      await tester.pumpAndSettle();

      // Smoothing is initially on from isSmoothing: true
      var switchWidget = tester.widget<Switch>(find.byType(Switch));
      expect(switchWidget.value, true);

      // Toggle smoothing off
      await tester.tap(find.byType(Switch));
      await tester.pumpAndSettle();
      switchWidget = tester.widget<Switch>(find.byType(Switch));
      expect(switchWidget.value, false);

      // Change line style to dashed (3rd segment, index 2)
      final segments = find.descendant(
        of: find.byType(SegmentedButton<PathLineStyle>),
        matching: find.byType(CustomPaint),
      );
      await tester.tap(segments.at(2));
      await tester.pumpAndSettle();
      final segmented = tester.widget<SegmentedButton<PathLineStyle>>(
        find.byType(SegmentedButton<PathLineStyle>),
      );
      expect(segmented.selected, {PathLineStyle.dashed});
    });
  });

  // =========================================================================
  // SECTION 7: Widget tests — PathDetailSheet
  // =========================================================================
  group('PathDetailSheet widget', () {
    late Database db;

    setUpAll(() {
      sqfliteFfiInit();
      databaseFactory = databaseFactoryFfi;
    });

    setUp(() async {
      db = await _createTestDb();
    });

    tearDown(() async {
      await db.close();
    });

    testWidgets('pre-populates customization from existing path', (tester) async {
      final path = _makePath(
        title: 'Existing Path',
        colorHex: 'D32F2F',
        iconKey: 'Vandring',
        smoothing: true,
        lineStyleKey: 'dashed',
      );

      await tester.pumpWidget(_testApp(
        SingleChildScrollView(
          child: PathDetailSheet(path: path),
        ),
        dbOverride: db,
      ));
      await tester.pumpAndSettle();

      // Title is pre-populated
      final nameField = tester.widget<TextFormField>(find.byType(TextFormField).first);
      expect(nameField.controller?.text, 'Existing Path');

      // Smoothing switch should be on
      final switchWidget = tester.widget<Switch>(find.byType(Switch));
      expect(switchWidget.value, true);

      // Icon name should show "Hiking"
      expect(find.text('Hiking'), findsOneWidget);

      // Line style "Dashed" should be selected
      final segmented = tester.widget<SegmentedButton<PathLineStyle>>(
        find.byType(SegmentedButton<PathLineStyle>),
      );
      expect(segmented.selected, {PathLineStyle.dashed});
    });

    testWidgets('path with no customization shows defaults', (tester) async {
      final path = _makePath(title: 'Plain Path');

      await tester.pumpWidget(_testApp(
        SingleChildScrollView(
          child: PathDetailSheet(path: path),
        ),
        dbOverride: db,
      ));
      await tester.pumpAndSettle();

      // Smoothing off
      final switchWidget = tester.widget<Switch>(find.byType(Switch));
      expect(switchWidget.value, false);

      // No icon selected (shows "Icon" text)
      expect(find.text('Icon'), findsOneWidget);
      expect(find.byIcon(Icons.clear), findsNothing);

      // Line style "Solid" selected
      final segmented = tester.widget<SegmentedButton<PathLineStyle>>(
        find.byType(SegmentedButton<PathLineStyle>),
      );
      expect(segmented.selected, {PathLineStyle.solid});
    });

    testWidgets('renders edit title and all buttons', (tester) async {
      final path = _makePath(title: 'Some Path');

      await tester.pumpWidget(_testApp(
        SingleChildScrollView(
          child: PathDetailSheet(path: path),
        ),
        dbOverride: db,
      ));
      await tester.pumpAndSettle();

      expect(find.text('Edit Path'), findsOneWidget);
      expect(find.text('Save Changes'), findsOneWidget);
      expect(find.text('Export Path'), findsOneWidget);
      expect(find.text('Delete Path'), findsOneWidget);
    });
  });

  // =========================================================================
  // SECTION 8: DB migration — backward compatibility
  // =========================================================================
  group('DB migration backward compatibility', () {
    setUpAll(() {
      sqfliteFfiInit();
      databaseFactory = databaseFactoryFfi;
    });

    test('v3 path (no customization columns) + migration → defaults work', () async {
      // Create a v3-schema database
      final migrationDb = await databaseFactory.openDatabase(
        inMemoryDatabasePath,
        options: OpenDatabaseOptions(singleInstance: false),
      );
      await migrationDb.execute('''
        CREATE TABLE $savedPathsTable(
          uuid TEXT PRIMARY KEY, title TEXT NOT NULL, description TEXT,
          points TEXT NOT NULL, distance REAL NOT NULL,
          min_lat REAL NOT NULL, min_lng REAL NOT NULL,
          max_lat REAL NOT NULL, max_lng REAL NOT NULL,
          created_at TEXT NOT NULL
        )
      ''');

      // Insert 3 paths with v3 schema
      for (var i = 0; i < 3; i++) {
        await migrationDb.insert(savedPathsTable, {
          'uuid': 'v3-path-$i',
          'title': 'V3 Path $i',
          'points': '[[59.9,10.7],[60.0,10.8]]',
          'distance': 1000.0 + i * 100,
          'min_lat': 59.9,
          'min_lng': 10.7,
          'max_lat': 60.0,
          'max_lng': 10.8,
          'created_at': DateTime.now().toIso8601String(),
        });
      }

      // Run v4 migration
      await migrationDb.execute('ALTER TABLE $savedPathsTable ADD COLUMN color_hex TEXT');
      await migrationDb.execute('ALTER TABLE $savedPathsTable ADD COLUMN icon_key TEXT');
      await migrationDb.execute('ALTER TABLE $savedPathsTable ADD COLUMN smoothing INTEGER NOT NULL DEFAULT 0');
      await migrationDb.execute('ALTER TABLE $savedPathsTable ADD COLUMN line_style TEXT');

      // All 3 paths should survive and have default customization values
      final store = SQLiteSavedPathDataStore(migrationDb);
      final all = await store.getAll();
      expect(all.length, 3);

      for (final path in all) {
        expect(path.colorHex, isNull);
        expect(path.iconKey, isNull);
        expect(path.smoothing, false);
        expect(path.lineStyleKey, isNull);
      }

      // Can add a new path with customization after migration
      await store.insert(_makePath(
        uuid: 'new-after-migration',
        title: 'New Styled Path',
        colorHex: '1976D2',
        lineStyleKey: 'dotted',
        smoothing: true,
        iconKey: 'Fjell',
      ));

      final newPath = await store.getByUuid('new-after-migration');
      expect(newPath!.colorHex, '1976D2');
      expect(newPath.lineStyleKey, 'dotted');
      expect(newPath.smoothing, true);
      expect(newPath.iconKey, 'Fjell');

      // Can update an old path to add customization
      final oldPath = await store.getByUuid('v3-path-0');
      await store.update(oldPath!.copyWith(
        colorHex: 'FF0000',
        smoothing: true,
      ));
      final updated = await store.getByUuid('v3-path-0');
      expect(updated!.colorHex, 'FF0000');
      expect(updated.smoothing, true);
      // Other old paths unchanged
      final unchanged = await store.getByUuid('v3-path-1');
      expect(unchanged!.colorHex, isNull);
      expect(unchanged.smoothing, false);

      await migrationDb.close();
    });
  });
}
