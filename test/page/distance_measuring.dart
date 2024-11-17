import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:latlong2/latlong.dart';
import 'package:map_app/widgets/map/measuring/measuring_controls.dart';
import 'package:map_app/widgets/map/measuring/measuring_map.dart';

void main() {
  testWidgets('Increase measuring distance by tapping', (WidgetTester tester) async {
   const initialPosition = LatLng(5, 5);
   const firstPoint = LatLng(5, 5);

   const double zoom = 5;

   const measuringPage = MeasuringControllerPage(initialPosition: initialPosition, startPoint: firstPoint, zoom: zoom);

   await tester.pumpWidget(const MaterialApp(home: ProviderScope(child: measuringPage)));

    // Verify that controls are displayed
    expect(find.byType(MeasuringControls), findsWidgets);

   expect(tester.widget(find.byType(MeasuringControls)),
       isA<MeasuringControls>().having((t) =>  t.distance, 'distance',  0.0));

   // Tap on the map
   await tester.tapAt(const Offset(100, 100));
   await tester.pumpAndSettle();

   expect(tester.widget(find.byType(MeasuringControls)),
       isA<MeasuringControls>().having((t) =>  t.distance, 'distance',  greaterThan(0.0)));
  });
}