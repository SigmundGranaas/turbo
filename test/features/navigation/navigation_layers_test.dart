import 'package:flutter/material.dart';
import 'package:flutter_map/flutter_map.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:latlong2/latlong.dart';
import 'package:turbo/core/location/location_state.dart';
import 'package:turbo/features/navigation/data/navigation_state.dart';
import 'package:turbo/features/navigation/data/navigation_state_notifier.dart';
import 'package:turbo/features/navigation/widgets/navigation_polyline_layer.dart';
import 'package:turbo/features/navigation/widgets/navigation_target_marker.dart';

import '../../helpers/pump_app.dart';

class _StubNav extends NavigationStateNotifier {
  _StubNav(this._initial);
  final NavigationState _initial;
  @override
  NavigationState build() => _initial;
}

class _StubLocation extends LocationState {
  _StubLocation(this._initial);
  final LatLng? _initial;
  @override
  Future<LatLng?> build() async => _initial;
}

Widget _mapHosting(Widget layer) => SizedBox(
      width: 600,
      height: 600,
      child: FlutterMap(
        options: const MapOptions(
          initialCenter: LatLng(59.9, 10.7),
          initialZoom: 8,
        ),
        children: [layer],
      ),
    );

void main() {
  group('NavigationPolylineLayer', () {
    testWidgets('renders an empty SizedBox when navigation is inactive',
        (tester) async {
      await pumpTestApp(
        tester,
        _mapHosting(const NavigationPolylineLayer()),
        overrides: [
          navigationStateProvider
              .overrideWith(() => _StubNav(NavigationState.inactive)),
          locationStateProvider
              .overrideWith(() => _StubLocation(const LatLng(59.9, 10.7))),
        ],
      );

      expect(find.byType(PolylineLayer), findsNothing);
    });

    testWidgets('renders a PolylineLayer when active and the user position '
        'is known', (tester) async {
      await pumpTestApp(
        tester,
        _mapHosting(const NavigationPolylineLayer()),
        overrides: [
          navigationStateProvider.overrideWith(() => _StubNav(
                const NavigationState(
                  target: LatLng(60.0, 11.0),
                  isActive: true,
                ),
              )),
          locationStateProvider
              .overrideWith(() => _StubLocation(const LatLng(59.9, 10.7))),
        ],
      );

      expect(find.byType(PolylineLayer), findsOneWidget);
    });

    testWidgets('renders nothing when active but the user position is unknown',
        (tester) async {
      await pumpTestApp(
        tester,
        _mapHosting(const NavigationPolylineLayer()),
        overrides: [
          navigationStateProvider.overrideWith(() => _StubNav(
                const NavigationState(
                  target: LatLng(60.0, 11.0),
                  isActive: true,
                ),
              )),
          locationStateProvider.overrideWith(() => _StubLocation(null)),
        ],
      );

      expect(find.byType(PolylineLayer), findsNothing);
    });
  });

  group('NavigationTargetMarker', () {
    testWidgets('renders nothing when navigation is inactive', (tester) async {
      await pumpTestApp(
        tester,
        _mapHosting(const NavigationTargetMarker()),
        overrides: [
          navigationStateProvider
              .overrideWith(() => _StubNav(NavigationState.inactive)),
        ],
      );

      expect(find.byType(MarkerLayer), findsNothing);
      expect(find.byIcon(Icons.flag_circle), findsNothing);
    });

    testWidgets(
        'renders a flag marker at the target position when active',
        (tester) async {
      await pumpTestApp(
        tester,
        _mapHosting(const NavigationTargetMarker()),
        overrides: [
          navigationStateProvider.overrideWith(() => _StubNav(
                const NavigationState(
                  target: LatLng(60.0, 11.0),
                  isActive: true,
                ),
              )),
        ],
      );

      expect(find.byType(MarkerLayer), findsOneWidget);
      expect(find.byIcon(Icons.flag_circle), findsOneWidget);
    });
  });
}
