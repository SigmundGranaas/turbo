import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:turbo/features/external_vector_layers/api.dart';

import '../../helpers/pump_app.dart';

VectorFeature _trailFeature(Map<String, Object?> props) {
  return VectorFeature(
    id: 'fotrute.1',
    kind: VectorGeometryKind.line,
    rings: const [],
    properties: props,
  );
}

Future<void> _pumpSheet(WidgetTester tester, VectorFeature feature,
    {Color accent = const Color(0xFFE63946),
    String subtype = 'Hiking trails',
    TrailProperties? decoded}) async {
  await tester.pumpWidget(buildTestApp(
    TrailFeatureSheet(
      feature: feature,
      subtypeLabel: subtype,
      accent: accent,
      decoded: decoded,
    ),
  ));
  await tester.pumpAndSettle();
}

Future<void> _pumpWithBuilder(
  WidgetTester tester,
  VectorFeature feature,
  TrailProperties Function(VectorFeature, BuildContext) builder, {
  Color accent = const Color(0xFF8E44AD),
  String subtype = 'OSM paths',
}) async {
  await tester.pumpWidget(buildTestApp(
    Builder(builder: (ctx) {
      return TrailFeatureSheet(
        feature: feature,
        subtypeLabel: subtype,
        accent: accent,
        decoded: builder(feature, ctx),
      );
    }),
  ));
  await tester.pumpAndSettle();
}

void main() {
  group('TrailFeatureSheet — emphasis & layout', () {
    testWidgets('hero is the real route name; subtype is the small label',
        (tester) async {
      await _pumpSheet(
        tester,
        _trailFeature({
          'rutenavn': 'Besseggen',
          'rutenummer': 'jot42',
        }),
      );
      expect(find.text('Besseggen'), findsOneWidget);
      // Subtype renders in uppercase above the title.
      expect(find.text('HIKING TRAILS'), findsOneWidget);
      expect(find.text('jot42'), findsOneWidget);
    });

    testWidgets('placeholder name "Ukjent" is replaced by the localised '
        'fallback, never shown raw', (tester) async {
      await _pumpSheet(
        tester,
        _trailFeature({'rutenavn': 'Ukjent'}),
      );
      expect(find.text('Ukjent'), findsNothing);
      expect(find.text('Unnamed route'), findsOneWidget);
    });
  });

  group('TrailFeatureSheet — SOSI code decoding', () {
    testWidgets('merking=JA → "Marked" chip', (tester) async {
      await _pumpSheet(
        tester,
        _trailFeature({'rutenavn': 'X', 'merking': 'JA'}),
      );
      expect(find.text('Marked'), findsOneWidget);
      // Raw code never reaches the UI.
      expect(find.text('JA'), findsNothing);
    });

    testWidgets('merking=SM → "Summer-marked"', (tester) async {
      await _pumpSheet(
        tester,
        _trailFeature({'rutenavn': 'X', 'merking': 'SM'}),
      );
      expect(find.text('Summer-marked'), findsOneWidget);
    });

    testWidgets('gradering=R → "Demanding" with red tint', (tester) async {
      await _pumpSheet(
        tester,
        _trailFeature({'rutenavn': 'X', 'gradering': 'R'}),
      );
      expect(find.text('Demanding'), findsOneWidget);
      expect(find.text('R'), findsNothing);
    });

    testWidgets('preparering=M → "Machine-groomed" (ski track)',
        (tester) async {
      await _pumpSheet(
        tester,
        _trailFeature({'rutenavn': 'X', 'preparering': 'M'}),
      );
      expect(find.text('Machine-groomed'), findsOneWidget);
    });
  });

  group('TrailFeatureSheet — detail-row hygiene', () {
    testWidgets('empty / null values do not produce blank detail rows',
        (tester) async {
      await _pumpSheet(
        tester,
        _trailFeature({
          'rutenavn': 'Besseggen',
          // Everything else missing or empty.
          'underlagstype': '',
          'sesong': '',
          'informasjon': null,
        }),
      );
      // Detail-row label words must not appear when their value is absent.
      expect(find.text('Surface'), findsNothing);
      expect(find.text('Notes'), findsNothing);
      expect(find.text('Maintained by'), findsNothing);
    });

    testWidgets('multi-maintainer pipe-list reflows to middot-separated text',
        (tester) async {
      await _pumpSheet(
        tester,
        _trailFeature({
          'rutenavn': 'X',
          'vedlikeholdsansvarlig': 'DNT | Lom kommune',
        }),
      );
      expect(find.text('DNT · Lom kommune'), findsOneWidget);
      expect(find.text('Maintained by'), findsOneWidget);
    });

    testWidgets('GUIDs, internal IDs, accuracy codes never make it onto the '
        'screen', (tester) async {
      await _pumpSheet(
        tester,
        _trailFeature({
          'rutenavn': 'X',
          'lokalId': 'e76cb78a-bf00-40d0-ba56-6efc13cafee3',
          'navnerom': 'http://data.geonorge.no/TurruterNGIS/Turruter/so',
          'versjonId': '2026-02-05 15:55:26.217713000',
          'målemetode': '55',
          'nøyaktighet': '1500',
        }),
      );
      expect(find.textContaining('e76cb78a'), findsNothing);
      expect(find.textContaining('geonorge.no/TurruterNGIS'), findsNothing);
      expect(find.textContaining('målemetode'), findsNothing);
      expect(find.text('55'), findsNothing);
    });

    testWidgets('source footer collapses date to yyyy-MM-dd', (tester) async {
      await _pumpSheet(
        tester,
        _trailFeature({
          'rutenavn': 'X',
          'opphav': 'N50-kartdata',
          'oppdateringsdato': '2026-02-05T14:55:26',
        }),
      );
      expect(find.textContaining('N50-kartdata'), findsOneWidget);
      expect(find.textContaining('2026-02-05'), findsOneWidget);
      // The time portion should be trimmed.
      expect(find.textContaining('14:55:26'), findsNothing);
    });
  });

  group('TrailFeatureSheet — OSM vocabulary', () {
    testWidgets('sac_scale=mountain_hiking → "Moderate" with blue tint',
        (tester) async {
      await _pumpWithBuilder(
        tester,
        _trailFeature({'name': 'Besseggen', 'sac_scale': 'mountain_hiking'}),
        TrailProperties.fromOsm,
      );
      expect(find.text('Besseggen'), findsOneWidget);
      expect(find.text('Moderate'), findsOneWidget);
      // Raw OSM value never leaks.
      expect(find.text('mountain_hiking'), findsNothing);
    });

    testWidgets('osmc:symbol presence implies marked', (tester) async {
      await _pumpWithBuilder(
        tester,
        _trailFeature({
          'name': 'X',
          'osmc:symbol': 'red:white:red_dot',
        }),
        TrailProperties.fromOsm,
      );
      expect(find.text('Marked'), findsOneWidget);
      // The raw symbol spec is never displayed.
      expect(find.textContaining('red:white'), findsNothing);
    });

    testWidgets('informal=yes is shown as unmarked', (tester) async {
      await _pumpWithBuilder(
        tester,
        _trailFeature({'name': 'X', 'informal': 'yes'}),
        TrailProperties.fromOsm,
      );
      expect(find.text('Unmarked'), findsOneWidget);
    });

    testWidgets('highway=path → "Sti" in the follows row', (tester) async {
      await _pumpWithBuilder(
        tester,
        _trailFeature({'name': 'X', 'highway': 'path'}),
        TrailProperties.fromOsm,
      );
      expect(find.text('Follows'), findsOneWidget);
      expect(find.text('Sti'), findsOneWidget);
    });

    testWidgets('piste:type implies a winter season', (tester) async {
      await _pumpWithBuilder(
        tester,
        _trailFeature({'name': 'X', 'piste:type': 'nordic'}),
        TrailProperties.fromOsm,
      );
      expect(find.text('Winter'), findsOneWidget);
    });

    testWidgets('OSM features attribute to OpenStreetMap in the footer',
        (tester) async {
      await _pumpWithBuilder(
        tester,
        _trailFeature({'name': 'X'}),
        TrailProperties.fromOsm,
      );
      expect(find.textContaining('OpenStreetMap'), findsOneWidget);
    });
  });

  group('TrailFeatureSheet — Kartverket FKB Traktorveg+Sti vocabulary', () {
    testWidgets('typeveg=sti → "Sti" in the follows row', (tester) async {
      await _pumpWithBuilder(
        tester,
        _trailFeature({
          'typeveg': 'sti',
          'kommunenummer': '3434',
        }),
        TrailProperties.fromN50Sti,
        accent: const Color(0xFF6D4C41),
        subtype: 'Paths (N50)',
      );
      expect(find.text('Sti'), findsOneWidget);
      expect(find.text('Follows'), findsOneWidget);
    });

    testWidgets('typeveg=traktorveg → "Traktorvei"', (tester) async {
      await _pumpWithBuilder(
        tester,
        _trailFeature({'typeveg': 'traktorveg'}),
        TrailProperties.fromN50Sti,
        accent: const Color(0xFF6D4C41),
        subtype: 'Paths (N50)',
      );
      expect(find.text('Traktorvei'), findsOneWidget);
    });

    testWidgets('FKB features never carry a name — fallback fires',
        (tester) async {
      await _pumpWithBuilder(
        tester,
        _trailFeature({'typeveg': 'sti'}),
        TrailProperties.fromN50Sti,
        accent: const Color(0xFF6D4C41),
        subtype: 'Paths (N50)',
      );
      expect(find.text('Unnamed route'), findsOneWidget);
    });

    testWidgets('source footer attributes to Kartverket FKB',
        (tester) async {
      await _pumpWithBuilder(
        tester,
        _trailFeature({'typeveg': 'sti'}),
        TrailProperties.fromN50Sti,
      );
      expect(find.textContaining('Kartverket FKB'), findsOneWidget);
    });
  });
}
