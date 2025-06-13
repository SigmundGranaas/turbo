import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:turbo/data/model/named_icon.dart';
import 'package:turbo/data/icon_service.dart';
import 'package:turbo/widgets/pages/icon_selection_page.dart';

void main() {
  late IconService mockIconService;

  // Helper to correctly pump the IconSelectionPage within a Navigator
  Future<void> pumpIconSelectionPage(WidgetTester tester, IconService service) async {
    await tester.pumpWidget(
      MaterialApp(
        home: Scaffold(
          body: Builder(
            builder: (context) {
              return Center(
                child: ElevatedButton(
                  child: const Text('Open'),
                  onPressed: () {
                    // Use the static show method which pushes the page
                    IconSelectionPage.show(context, service);
                  },
                ),
              );
            },
          ),
        ),
      ),
    );

    // Tap the button to show the IconSelectionPage
    await tester.tap(find.byType(ElevatedButton));
    await tester.pumpAndSettle();

    // Verify the page is now visible
    expect(find.byType(IconSelectionPage), findsOneWidget);
  }

  setUp(() {
    mockIconService = MockIconService();
  });

  testWidgets('Success case: Select an icon without searching', (WidgetTester tester) async {
    await pumpIconSelectionPage(tester, mockIconService);

    // Verify that icons are displayed
    expect(find.byType(IconGridItem), findsWidgets);

    // Tap on the first icon
    await tester.tap(find.byType(IconGridItem).first);
    await tester.pumpAndSettle();

    // Verify that the page is closed
    expect(find.byType(IconSelectionPage), findsNothing);
  });

  testWidgets('Success case: Search for an icon and select it', (WidgetTester tester) async {
    await pumpIconSelectionPage(tester, mockIconService);

    // Enter search text into the SearchBar
    await tester.enterText(find.byType(SearchBar), 'Home');
    await tester.pump(); // pump to reflect the changes

    // Verify that search results are displayed
    expect(find.byType(IconGridItem), findsWidgets);

    // Tap on the first search result
    await tester.tap(find.byType(IconGridItem).first);
    await tester.pumpAndSettle();

    // Verify that the page is closed
    expect(find.byType(IconSelectionPage), findsNothing);
  });

  testWidgets('Neutral case: Go back without selecting an icon', (WidgetTester tester) async {
    await pumpIconSelectionPage(tester, mockIconService);

    // Tap the back button in the AppBar
    await tester.tap(find.byIcon(Icons.arrow_back));
    await tester.pumpAndSettle();

    // Verify that the page is closed
    expect(find.byType(IconSelectionPage), findsNothing);
  });

  testWidgets('Failure case: Search with no results', (WidgetTester tester) async {
    await pumpIconSelectionPage(tester, mockIconService);

    // Enter search text that should yield no results
    await tester.enterText(find.byType(SearchBar), 'nonexistenticon');
    await tester.pump();

    // Verify that no search results are displayed
    expect(find.byType(IconGridItem), findsNothing);

    // Check for the correct "no results" message
    expect(find.text('No icons found.'), findsOneWidget);
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