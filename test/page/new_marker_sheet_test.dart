import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:map_app/widgets/marker/create_location_sheet.dart';
import 'package:map_app/widgets/pages/icon_selection_page.dart';
import 'package:provider/provider.dart';
import 'package:latlong2/latlong.dart';
import 'package:map_app/data/model/marker.dart';
import 'package:map_app/location_provider.dart';

class TestLocationProvider extends LocationProvider {
  List<Marker> markers = [];

  @override
  Future<Marker> addLocation(Marker marker) async {
    final newMarker = Marker.fromMap({
      ...marker.toMap(),
      'id': DateTime.now().millisecondsSinceEpoch.toString(),
    });
    markers.add(newMarker);
    notifyListeners();
    return newMarker;
  }

  @override
  Future<List<Marker>> loadLocations() async {
    return markers;
  }
}

void main() {
  late TestLocationProvider locationProvider;

  setUp(() {
    locationProvider = TestLocationProvider();
  });

  Widget createWidgetUnderTest({LatLng? newLocation}) {
    return MaterialApp(
      home: Scaffold(
        body: ChangeNotifierProvider<LocationProvider>.value(
          value: locationProvider,
          child: CreateLocationSheet(newLocation: newLocation),
        ),
      ),
    );
  }

  testWidgets('Success test: Create a marker with icon, name, and description', (WidgetTester tester) async {
    await tester.pumpWidget(createWidgetUnderTest(newLocation: const LatLng(0, 0)));

    await tester.enterText(find.byType(TextFormField).first, 'Test Location');
    await tester.enterText(find.byType(TextFormField).last, 'Test Description');

    // Simulate icon selection
    await tester.tap(find.byType(ListTile));
    await tester.pumpAndSettle();
    await tester.tap(find.byType(IconGridItem).first);
    await tester.pumpAndSettle();

    await tester.tap(find.text('Lagre'));
    await tester.pumpAndSettle();

    expect(locationProvider.markers.length, 1);
    expect(locationProvider.markers.first.title, 'Test Location');
    expect(locationProvider.markers.first.description, 'Test Description');
    expect(find.byType(CreateLocationSheet), findsNothing);
  });

  testWidgets('Success test: Create a marker with only name', (WidgetTester tester) async {
    await tester.pumpWidget(createWidgetUnderTest(newLocation: const LatLng(0, 0)));

    await tester.enterText(find.byType(TextFormField).first, 'Test Location');
    await tester.tap(find.text('Lagre'));
    await tester.pumpAndSettle();

    expect(locationProvider.markers.length, 1);
    expect(locationProvider.markers.first.title, 'Test Location');
    expect(locationProvider.markers.first.description, '');
    expect(find.byType(CreateLocationSheet), findsNothing);
  });

  testWidgets('Close sheet when clicking X button', (WidgetTester tester) async {
    await tester.pumpWidget(createWidgetUnderTest());

    await tester.tap(find.byIcon(Icons.close));
    await tester.pumpAndSettle();

    expect(find.byType(CreateLocationSheet), findsNothing);
  });

  testWidgets('Show error when trying to save without a name', (WidgetTester tester) async {
    await tester.pumpWidget(createWidgetUnderTest(newLocation: const LatLng(0, 0)));

    await tester.tap(find.text('Lagre'));
    await tester.pumpAndSettle();

    expect(find.text('Skriv inn et navn'), findsOneWidget);
  });
}