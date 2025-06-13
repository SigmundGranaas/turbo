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