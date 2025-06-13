import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:turbo/data/model/named_icon.dart';
import 'package:turbo/data/icon_service.dart';
import 'package:turbo/widgets/pages/icon_selection_page.dart';

void main() {
  late IconService mockIconService;

  setUp(() {
    mockIconService = MockIconService();
  });

  testWidgets('Success case: Select an icon without searching', (WidgetTester tester) async {
    await tester.pumpWidget(MaterialApp(home: IconSelectionPage(iconService: mockIconService)));

    // Verify that icons are displayed
    expect(find.byType(IconGridItem), findsWidgets);

    // Tap on the first icon
    await tester.tap(find.byType(IconGridItem).first);
    await tester.pumpAndSettle();

    // Verify that the correct icon was returned
    expect(tester.takeException(), null);
    expect(find.byType(IconSelectionPage), findsNothing);
  });

  testWidgets('Success case: Search for an icon and select it', (WidgetTester tester) async {
    await tester.pumpWidget(MaterialApp(home: IconSelectionPage(iconService: mockIconService)));

    // Enter search text
    await tester.enterText(find.byType(TextField), 'Home');
    await tester.pump();

    // Verify that search results are displayed
    expect(find.byType(IconGridItem), findsWidgets);

    // Tap on the first search result
    await tester.tap(find.byType(IconGridItem).first);
    await tester.pumpAndSettle();

    // Verify that the correct icon was returned
    expect(tester.takeException(), null);
    expect(find.byType(IconSelectionPage), findsNothing);
  });

  testWidgets('Neutral case: Go back without selecting an icon', (WidgetTester tester) async {
    await tester.pumpWidget(MaterialApp(home: IconSelectionPage(iconService: mockIconService)));

    // Tap the back button
    await tester.tap(find.byIcon(Icons.arrow_back));
    await tester.pumpAndSettle();

    // Verify that no icon was returned and the page is closed
    expect(tester.takeException(), null);
    expect(find.byType(IconSelectionPage), findsNothing);
  });

  testWidgets('Failure case: Search with no results', (WidgetTester tester) async {
    await tester.pumpWidget(MaterialApp(home: IconSelectionPage(iconService: mockIconService)));

    // Enter search text that should yield no results
    await tester.enterText(find.byType(TextField), 'nonexistenticon');
    await tester.pump();

    // Verify that no search results are displayed
    expect(find.byType(IconGridItem), findsNothing);

    expect(find.text('Ingen resultater'), findsOneWidget);
  });
}

class MockIconService extends IconService {

  final Map<String, NamedIcon> _icons = {
    'Home': const NamedIcon(icon: Icons.home, title: 'Home'),
    'Settings': const NamedIcon(icon: Icons.settings, title: 'Settings'),
    'Person':  const NamedIcon(icon: Icons.person, title: 'Person'),
  };

  @override
  NamedIcon getIcon(String? title) {
    return _icons[title] ?? const NamedIcon(title: 'Default', icon: Icons.help_outline);
  }

  @override
  List<NamedIcon> getAllIcons() {
    return _icons.values.toList();
  }
}