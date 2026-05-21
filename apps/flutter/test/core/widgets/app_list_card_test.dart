import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:turbo/core/widgets/app_list_card.dart';

Widget _harness(Widget child) => MaterialApp(home: Scaffold(body: child));

void main() {
  group('AppListCard', () {
    testWidgets('renders the icon and title', (tester) async {
      await tester.pumpWidget(_harness(
        const AppListCard(icon: Icons.share, title: 'Share as text'),
      ));
      expect(find.byIcon(Icons.share), findsOneWidget);
      expect(find.text('Share as text'), findsOneWidget);
    });

    testWidgets('renders the optional subtitle when provided', (tester) async {
      await tester.pumpWidget(_harness(
        const AppListCard(
          icon: Icons.data_object,
          title: 'GeoJSON',
          subtitle: 'Works with GIS tools',
        ),
      ));
      expect(find.text('Works with GIS tools'), findsOneWidget);
    });

    testWidgets('omits the subtitle paragraph when null', (tester) async {
      await tester.pumpWidget(_harness(
        const AppListCard(icon: Icons.share, title: 'Title only'),
      ));
      expect(find.text('Title only'), findsOneWidget);
      // The body should contain exactly one Text widget — the title.
      expect(find.byType(Text), findsOneWidget);
    });

    testWidgets('renders trailing widgets after the title column',
        (tester) async {
      await tester.pumpWidget(_harness(
        AppListCard(
          icon: Icons.route,
          title: 'GPX',
          trailing: [
            IconButton(
              icon: const Icon(Icons.share),
              onPressed: () {},
            ),
            IconButton(
              icon: const Icon(Icons.save_alt),
              onPressed: () {},
            ),
          ],
        ),
      ));
      expect(find.byIcon(Icons.share), findsOneWidget);
      expect(find.byIcon(Icons.save_alt), findsOneWidget);
    });

    testWidgets('forwards taps via onTap and wraps content in InkWell',
        (tester) async {
      var taps = 0;
      await tester.pumpWidget(_harness(
        AppListCard(
          icon: Icons.grid_view,
          title: 'Choose icon',
          onTap: () => taps++,
        ),
      ));
      // Tap-target uses the whole card.
      await tester.tap(find.byType(AppListCard));
      expect(taps, 1);
      // Ink ripple is set up.
      expect(find.byType(InkWell), findsOneWidget);
    });

    testWidgets('does NOT wrap in InkWell when onTap is null',
        (tester) async {
      await tester.pumpWidget(_harness(
        const AppListCard(icon: Icons.share, title: 'Static row'),
      ));
      expect(find.byType(InkWell), findsNothing);
    });
  });
}
