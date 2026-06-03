import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:latlong2/latlong.dart';
import 'package:turbo/core/geo/geo_path.dart';
import 'package:turbo/core/location/follow_mode_state.dart';
import 'package:turbo/core/location/location_state.dart';
import 'package:turbo/features/journey/api.dart';
import 'package:turbo/features/navigation/api.dart';

class _FixedLocation extends LocationState {
  _FixedLocation(this._pos);
  final LatLng _pos;
  @override
  Future<LatLng?> build() async => _pos;
}

void main() {
  late ProviderContainer container;

  setUp(() => container = ProviderContainer());
  tearDown(() => container.dispose());

  GeoPath samplePath() => GeoPath.fromPoints(
        const [LatLng(60.0, 10.0), LatLng(60.0, 10.01), LatLng(60.0, 10.02)],
        source: GeoPathSource.trail,
      );

  test('starts inactive', () {
    expect(container.read(activeJourneyProvider).kind, JourneyKind.none);
    expect(container.read(activeJourneyProvider).isActive, isFalse);
  });

  test('navigateToPoint sets state and engages follow mode', () {
    container
        .read(activeJourneyProvider.notifier)
        .navigateToPoint(const LatLng(61, 11));

    final j = container.read(activeJourneyProvider);
    expect(j.kind, JourneyKind.navigatingToPoint);
    expect(j.target, const LatLng(61, 11));
    expect(container.read(followModeProvider).isOn, isTrue);
  });

  test('navigationStateProvider projects navigatingToPoint', () {
    container
        .read(activeJourneyProvider.notifier)
        .navigateToPoint(const LatLng(61, 11));

    final nav = container.read(navigationStateProvider);
    expect(nav.isActive, isTrue);
    expect(nav.target, const LatLng(61, 11));
  });

  test('navigation notifier delegates to the journey', () {
    container
        .read(navigationStateProvider.notifier)
        .startNavigation(const LatLng(62, 12));
    expect(container.read(activeJourneyProvider).kind,
        JourneyKind.navigatingToPoint);

    container.read(navigationStateProvider.notifier).stopNavigation();
    expect(container.read(activeJourneyProvider).kind, JourneyKind.none);
    expect(container.read(navigationStateProvider).isActive, isFalse);
  });

  test('followPath without recording sets followingPath', () async {
    await container
        .read(activeJourneyProvider.notifier)
        .followPath(samplePath(), label: 'Trail X');

    final j = container.read(activeJourneyProvider);
    expect(j.kind, JourneyKind.followingPath);
    expect(j.label, 'Trail X');
    expect(j.recording, isFalse);
    expect(j.path, isNotNull);
    // While following a path, point-navigation projection stays inactive.
    expect(container.read(navigationStateProvider).isActive, isFalse);
  });

  test('stop clears the journey and disengages follow', () async {
    await container
        .read(activeJourneyProvider.notifier)
        .followPath(samplePath());
    container.read(activeJourneyProvider.notifier).stop();

    expect(container.read(activeJourneyProvider).kind, JourneyKind.none);
    expect(container.read(followModeProvider).isOn, isFalse);
  });

  // Regression: following a path whose end is already within arrival range
  // (so journeyProgress reports arrival on the first sample) used to crash with
  // "NavigationStateNotifier rebuilt multiple times in the same frame" — the
  // ActiveOutingPanel arrival listener finished the journey synchronously inside
  // the same provider flush that the journey-derived navigation projection was
  // rebuilding in. The panel now defers the finish to a microtask; this proves
  // the arrival → stop path runs cleanly with the projection kept alive.
  test('deferred arrival-finish ends the journey without re-entrant rebuild',
      () async {
    const end = LatLng(60.0, 10.02);
    final c = ProviderContainer(overrides: [
      locationStateProvider.overrideWith(() => _FixedLocation(end)),
    ]);
    addTearDown(c.dispose);

    // Keep the navigation projection (the provider that crashed) alive.
    c.listen(navigationStateProvider, (_, _) {}, fireImmediately: true);

    // Mirror ActiveOutingPanel: on arrival, finish the journey — deferred out of
    // the notification flush, exactly as the widget now does.
    var handled = false;
    c.listen(journeyProgressProvider, (_, next) {
      if (next != null && next.remainingM < 30 && !handled) {
        handled = true;
        Future.microtask(() => c.read(activeJourneyProvider.notifier).stop());
      }
    });

    await c.read(activeJourneyProvider.notifier).followPath(samplePath());
    // Let the location resolve + the deferred finish run.
    await Future<void>.delayed(Duration.zero);
    await Future<void>.delayed(Duration.zero);

    expect(handled, isTrue, reason: 'arrival should have been detected');
    expect(c.read(activeJourneyProvider).kind, JourneyKind.none,
        reason: 'arrival finishes the journey');
    expect(c.read(navigationStateProvider).isActive, isFalse);
  });

  test('destination resolves to path end / target', () async {
    final notifier = container.read(activeJourneyProvider.notifier);
    notifier.navigateToPoint(const LatLng(61, 11));
    expect(container.read(activeJourneyProvider).destination,
        const LatLng(61, 11));

    await notifier.followPath(samplePath());
    expect(container.read(activeJourneyProvider).destination,
        const LatLng(60.0, 10.02));
  });
}
