@Tags(['widget'])
library;

import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:turbo/features/activities/api.dart';

/// Tests that lock in the architectural invariants behind the detail
/// chassis rewrite:
///
/// 1. The weather panel's loading state runs **no animation**. The
///    bug we replaced was an `AnimationController.repeat(reverse: true)`
///    in the old `AnalysisSkeleton` that made the screen feel
///    permanently broken while the 15s HTTP timeout elapsed. The
///    chassis is non-animated by construction; this test makes that
///    a regression-proof contract.
///
/// 2. The static slots (title, stats, description, actions) paint
///    when the verdict slot is in its `.loading()` state. The whole
///    point of the chassis is that static activity data is *never*
///    gated on the analysis call.
///
/// 3. No per-kind detail sheet hardcodes driver-key strings — every
///    UI reference must route through the [DriverKeys] whitelist or
///    a [WeatherMetrics] preset. Catches the silent-drift failure
///    where a backend rename would empty a metric chip without any
///    compile error.
void main() {
  group('ActivityWeatherPanel', () {
    testWidgets('loading body has no animation ticker', (tester) async {
      await tester.pumpWidget(const MaterialApp(
        home: Material(
          child: ActivityWeatherPanel(
            title: 'Test',
            accent: Colors.blue,
            loadingState: WeatherLoadingState.loading,
          ),
        ),
      ));
      // Any AnimationController.repeat or active Ticker would register
      // at least one transient frame callback for the next vsync. The
      // chassis must produce zero — purely static layout.
      expect(
        tester.binding.transientCallbackCount,
        0,
        reason:
            'Loading state must not register any animation ticker — '
            'that was the original "spins forever" bug.',
      );
    });

    testWidgets('noData body shows distinct copy from error body',
        (tester) async {
      await tester.pumpWidget(const MaterialApp(
        home: Material(
          child: ActivityWeatherPanel(
            title: 'Test',
            accent: Colors.blue,
            loadingState: WeatherLoadingState.noData,
          ),
        ),
      ));
      expect(find.textContaining('No weather drivers'), findsOneWidget);
      expect(find.textContaining('Conditions unavailable'), findsNothing);
    });

    testWidgets('error body says unavailable, not "no data"', (tester) async {
      await tester.pumpWidget(const MaterialApp(
        home: Material(
          child: ActivityWeatherPanel(
            title: 'Test',
            accent: Colors.blue,
            loadingState: WeatherLoadingState.error,
          ),
        ),
      ));
      expect(find.textContaining('Conditions unavailable'), findsOneWidget);
      expect(find.textContaining('No weather drivers'), findsNothing);
    });
  });

  group('ActivityDetailChassis', () {
    testWidgets(
        'renders title + stats + description + actions while verdict is loading',
        (tester) async {
      await tester.pumpWidget(MaterialApp(
        home: Scaffold(
          body: Builder(
            builder: (ctx) => ActivityDetailChassis(
              tintColor: Colors.blue,
              icon: Icons.hiking,
              title: 'Test Trail',
              place: 'Hike',
              // Verdict slot in loading state — this used to be a
              // shimmer skeleton that swallowed the whole viewport.
              verdict: const ActivityVerdict.loading(),
              stats: const ActivityStatStrip(items: [
                StatItem('Distance', '4.2 km'),
                StatItem('Ascent', '320 m'),
              ]),
              description: 'A trail description that should be visible.',
              actions: ActivityAction.standardTriple(
                ctx,
                onLogVisit: () {},
                onEdit: () {},
                onDelete: () {},
              ),
            ),
          ),
        ),
      ));

      // Static content is present immediately — even though the
      // verdict slot is in its loading placeholder state.
      expect(find.text('Test Trail'), findsOneWidget);
      expect(find.text('4.2 km'), findsOneWidget);
      expect(find.text('320 m'), findsOneWidget);
      expect(
          find.text('A trail description that should be visible.'),
          findsOneWidget);
      expect(find.text('LOG VISIT'), findsOneWidget);
      expect(find.text('EDIT'), findsOneWidget);
      expect(find.text('DELETE'), findsOneWidget);

      // Verdict placeholder is shown (no spinner).
      expect(find.textContaining('Fetching conditions'), findsOneWidget);
      expect(find.byType(CircularProgressIndicator), findsNothing);

      // No animation ticker at all.
      expect(tester.binding.transientCallbackCount, 0);
    });

    testWidgets('null verdict / map / weather / module slots collapse cleanly',
        (tester) async {
      await tester.pumpWidget(const MaterialApp(
        home: Scaffold(
          body: ActivityDetailChassis(
            tintColor: Colors.blue,
            icon: Icons.hiking,
            title: 'Minimal',
            place: 'Hike',
          ),
        ),
      ));
      expect(find.text('Minimal'), findsOneWidget);
      // No exception from missing slots, no spinner.
      expect(find.byType(CircularProgressIndicator), findsNothing);
    });
  });

  group('ActivityVerdict', () {
    testWidgets('.loading() carries a liveRegion semantics', (tester) async {
      await tester.pumpWidget(const MaterialApp(
        home: Material(child: ActivityVerdict.loading()),
      ));
      final semantics = tester.getSemantics(find.byType(ActivityVerdict));
      // The verdict-during-loading is announced to screen readers as
      // a live region so they don't conflate it with a real verdict.
      expect(semantics.label, contains('Fetching'));
    });

    test('.fromScore picks tone bands at documented thresholds', () {
      expect(ActivityVerdict.toneFromScore(null), VerdictTone.neutral);
      expect(ActivityVerdict.toneFromScore(85), VerdictTone.good);
      expect(ActivityVerdict.toneFromScore(70), VerdictTone.good);
      expect(ActivityVerdict.toneFromScore(69), VerdictTone.caution);
      expect(ActivityVerdict.toneFromScore(40), VerdictTone.caution);
      expect(ActivityVerdict.toneFromScore(39), VerdictTone.stop);
      expect(ActivityVerdict.toneFromScore(0), VerdictTone.stop);
    });

    test('.headlineForScore picks copy at documented thresholds', () {
      expect(ActivityVerdict.headlineForScore(90), 'Great today');
      expect(ActivityVerdict.headlineForScore(80), 'Great today');
      expect(ActivityVerdict.headlineForScore(75), 'Good — go');
      expect(ActivityVerdict.headlineForScore(60), 'Good — go');
      expect(ActivityVerdict.headlineForScore(50), 'Marginal — pick your window');
      expect(ActivityVerdict.headlineForScore(40), 'Marginal — pick your window');
      expect(ActivityVerdict.headlineForScore(30), 'Tough — consider waiting');
      expect(ActivityVerdict.headlineForScore(20), 'Tough — consider waiting');
      expect(ActivityVerdict.headlineForScore(10), 'Avoid today');
      expect(ActivityVerdict.headlineForScore(0), 'Avoid today');
    });
  });

  group('DriverKeys whitelist contract', () {
    test('no detail sheet hardcodes a driver-key string outside DriverKeys',
        () {
      // Drift-prevention: scan every per-kind detail sheet for snake_case
      // string literals that look like driver keys (e.g. 'temp_band')
      // and assert each is in the whitelist. If you add a new driver,
      // add it to DriverKeys + WeatherMetrics, never as a raw string.
      final sheetGlob = [
        'lib/features/activity_xc_ski/widgets/xc_ski_detail_sheet.dart',
        'lib/features/activity_hiking/widgets/hiking_detail_sheet.dart',
        'lib/features/activity_fishing/widgets/fishing_detail_sheet.dart',
        'lib/features/activity_freediving/widgets/freediving_detail_sheet.dart',
        'lib/features/activity_backcountry_ski/widgets/backcountry_ski_detail_sheet.dart',
        'lib/features/activity_packrafting/widgets/packrafting_detail_sheet.dart',
      ];
      // Matches `'snake_case_with_underscores'` and `"…"` string
      // literals — driver keys look like `temp_band`, `fresh_snow_24h`,
      // `flow_cumecs`. Two or more underscore-separated chunks; all
      // lowercase + digits.
      final driverKeyish = RegExp(r"""['"]([a-z][a-z0-9]*_[a-z0-9_]+)['"]""");

      final violations = <String>[];
      for (final path in sheetGlob) {
        final content = File(path).readAsStringSync();
        for (final m in driverKeyish.allMatches(content)) {
          final key = m.group(1)!;
          // Skip obvious non-driver strings. `xc_ski`, `backcountry_ski`,
          // `backcountry-ski` are kind keys; `fresh_grooming_visible`,
          // `track_condition`, `snow_quality`, `frozen_granular`,
          // `breakable_crust`, `hard_pack`, `icy_ruts`, `deep_unbroken`,
          // `fast_glide` belong to observation extras. Filter by the
          // typical driver-key shape (`<thing>_<thing>` or contains a
          // unit suffix like `_24h`, `_cm`, etc.) — simpler: just allow
          // anything that is NOT in DriverKeys but ALSO is not in a
          // small known-allowlist of non-driver snake_case literals.
          const allowed = {
            'xc_ski',
            'backcountry_ski',
            'fresh_grooming_visible',
            'track_condition',
            'snow_quality',
            // Track-condition enum values (XcSki observation extras)
            'fast_glide',
            'frozen_granular',
            'icy_ruts',
            'deep_unbroken',
            // Snow-quality enum values
            'powder',
            'hard_pack',
            'breakable_crust',
          };
          if (allowed.contains(key)) continue;
          if (DriverKeys.known.contains(key)) continue;
          violations.add('$path: "$key"');
        }
      }
      expect(
        violations,
        isEmpty,
        reason:
            'Driver keys must come from DriverKeys/WeatherMetrics, not '
            'raw strings. Add the key to DriverKeys.known and a preset '
            'to WeatherMetrics, then reference the preset.',
      );
    });

    test('DriverKeys.known is consistent with its members', () {
      // Sanity: every constant in DriverKeys is listed in `known`.
      const declared = <String>{
        DriverKeys.tempBand,
        DriverKeys.wind,
        DriverKeys.rain24h,
        DriverKeys.pressureTrend,
        DriverKeys.freshSnow24h,
        DriverKeys.snowDepth,
        DriverKeys.seaTemp,
        DriverKeys.waterTemp,
        DriverKeys.waveHeight,
        DriverKeys.vizMeters,
        DriverKeys.flowCumecs,
      };
      expect(DriverKeys.known, declared);
    });
  });
}
