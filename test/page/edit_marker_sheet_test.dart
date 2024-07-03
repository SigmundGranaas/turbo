import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:map_app/widgets/marker/edit_location_sheet.dart';
import 'package:map_app/widgets/pages/icon_selection_page.dart';
import 'package:provider/provider.dart';
import 'package:map_app/data/model/marker.dart';
import 'package:map_app/location_provider.dart';

class TestLocationProvider extends LocationProvider {
  List<Marker> markers = [];

  @override
  Future<Marker> updateLocation(Marker marker) async {
    final index = markers.indexWhere((m) => m.uuid == marker.uuid);
    if (index != -1) {
      markers[index] = marker;
      notifyListeners();
      return marker;
    }
    throw Exception('Marker not found');
  }

  @override
  Future<void> deleteLocation(String id) async {
    markers.removeWhere((m) => m.uuid == id);
    notifyListeners();
  }

  @override
  Future<List<Marker>> loadLocations() async {
    return markers;
  }
}

void main() {
  late TestLocationProvider locationProvider;
  late Marker testMarker;

  setUp(() {
    locationProvider = TestLocationProvider();
    testMarker = Marker.fromMap({
      'id': '1',
      'title': 'Test Location',
      'description': 'Test Description',
      'icon': 'Fjell',
      'latitude': 0.0,
      'longitude': 0.0,
    });
    locationProvider.markers = [testMarker];
  });

  Widget createWidgetUnderTest() {
    return MaterialApp(
      home: Scaffold(
        body: ChangeNotifierProvider<LocationProvider>.value(
          value: locationProvider,
          child: EditLocationSheet(location: testMarker),
        ),
      ),
    );
  }

  testWidgets('Success test: Edit a marker with new name, description, and icon', (WidgetTester tester) async {
    await tester.pumpWidget(createWidgetUnderTest());

    await tester.enterText(find.byType(TextFormField).first, 'Updated Location');
    await tester.enterText(find.byType(TextFormField).last, 'Updated Description');

    // Simulate icon selection
    await tester.tap(find.byType(ListTile));
    await tester.pumpAndSettle();
    await tester.tap(find.byType(IconGridItem).first);
    await tester.pumpAndSettle();

    await tester.tap(find.text('Lagre'));
    await tester.pumpAndSettle();

    expect(locationProvider.markers.length, 1);
    expect(locationProvider.markers.first.title, 'Updated Location');
    expect(locationProvider.markers.first.description, 'Updated Description');
    expect(find.byType(EditLocationSheet), findsNothing);
  });

  testWidgets('Success test: Edit a marker with only new name', (WidgetTester tester) async {
    await tester.pumpWidget(createWidgetUnderTest());

    await tester.enterText(find.byType(TextFormField).first, 'Updated Location');
    await tester.tap(find.text('Lagre'));
    await tester.pumpAndSettle();

    expect(locationProvider.markers.length, 1);
    expect(locationProvider.markers.first.title, 'Updated Location');
    expect(locationProvider.markers.first.description, 'Test Description');
    expect(find.byType(EditLocationSheet), findsNothing);
  });

  testWidgets('Success test: Delete a marker', (WidgetTester tester) async {
    await tester.pumpWidget(createWidgetUnderTest());

    await tester.tap(find.text('Slett'));
    await tester.pumpAndSettle();

    expect(locationProvider.markers.length, 0);
    expect(find.byType(EditLocationSheet), findsNothing);
  });

  testWidgets('Close sheet when clicking X button', (WidgetTester tester) async {
    await tester.pumpWidget(createWidgetUnderTest());

    await tester.tap(find.byIcon(Icons.close));
    await tester.pumpAndSettle();

    expect(find.byType(EditLocationSheet), findsNothing);
  });

  testWidgets('Show error when trying to save without a name', (WidgetTester tester) async {
    await tester.pumpWidget(createWidgetUnderTest());

    await tester.enterText(find.byType(TextFormField).first, '');
    await tester.tap(find.text('Lagre'));
    await tester.pumpAndSettle();

    expect(find.text('Skriv inn et navn'), findsOneWidget);
  });
}