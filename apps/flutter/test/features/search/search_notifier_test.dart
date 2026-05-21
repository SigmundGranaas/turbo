import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:latlong2/latlong.dart';
import 'package:turbo/features/search/data/composite_search_service.dart';
import 'package:turbo/features/search/data/location_service.dart';
import 'package:turbo/features/search/data/search_state_provider.dart';

import '../../helpers/fakes/fake_location_service.dart';

LocationSearchResult _result(String title) => LocationSearchResult(
      title: title,
      position: const LatLng(0, 0),
      source: 'fake',
    );

ProviderContainer _container(LocationService service) {
  final container = ProviderContainer(overrides: [
    compositeSearchServiceProvider.overrideWithValue(
      // Re-use the FakeLocationService where the composite would normally sit.
      // The composite is itself a LocationService, so a fake stand-in is fine.
      _AsCompositeService(service),
    ),
  ]);
  // searchProvider is autoDispose — without an active listener, the debounce
  // timer is cancelled in ref.onDispose before it fires. A no-op listener
  // pins the subscription for the duration of the test.
  container.listen<AsyncValue<List<LocationSearchResult>>>(
    searchProvider,
    (_, _) {},
    fireImmediately: true,
  );
  addTearDown(container.dispose);
  return container;
}

/// Thin wrapper so we can pass a [FakeLocationService] where
/// [CompositeSearchService] is expected — only the [LocationService] contract
/// is exercised by [SearchNotifier].
class _AsCompositeService extends CompositeSearchService {
  final LocationService inner;
  _AsCompositeService(this.inner) : super(inner, inner, inner, inner);

  @override
  Future<List<LocationSearchResult>> findLocationsBy(String name) =>
      inner.findLocationsBy(name);
}

void main() {
  group('SearchNotifier.search', () {
    test('returns empty data without calling the service for queries < 2 chars',
        () async {
      final service = FakeLocationService(results: [_result('Should not appear')]);
      final container = _container(service);

      final notifier = container.read(searchProvider.notifier);
      await notifier.search('');
      expect(container.read(searchProvider).value, isEmpty);
      await notifier.search('a');
      expect(container.read(searchProvider).value, isEmpty);
      expect(service.queries, isEmpty);
    });

    test('transitions loading → data and calls the underlying service exactly once',
        () async {
      final service = FakeLocationService(results: [_result('Oslo')]);
      final container = _container(service);
      final notifier = container.read(searchProvider.notifier);

      notifier.search('oslo');
      // Synchronously after the call, state is AsyncLoading until the
      // debounce timer + future resolves.
      expect(container.read(searchProvider), isA<AsyncLoading>());

      // Debounce is 400 ms — wait for it to fire.
      await Future.delayed(const Duration(milliseconds: 500));
      final state = container.read(searchProvider);
      expect(state, isA<AsyncData<List<LocationSearchResult>>>());
      expect(state.value!.map((r) => r.title).toList(), ['Oslo']);
      expect(service.queries, ['oslo']);
    });

    test('rapid second call cancels the in-flight debounce — only the last '
        'query reaches the service', () async {
      final service = FakeLocationService(
        responder: (q) async => [_result(q.toUpperCase())],
      );
      final container = _container(service);
      final notifier = container.read(searchProvider.notifier);

      notifier.search('be');
      // Less than the 400 ms debounce later, a new query replaces it.
      await Future.delayed(const Duration(milliseconds: 100));
      notifier.search('bergen');
      await Future.delayed(const Duration(milliseconds: 500));

      expect(service.queries, ['bergen']);
      expect(
          container.read(searchProvider).value!.map((r) => r.title).toList(),
          ['BERGEN']);
    });

    test('service errors propagate as AsyncError', () async {
      final service = FakeLocationService(throwOnQuery: StateError('boom'));
      final container = _container(service);
      final notifier = container.read(searchProvider.notifier);

      notifier.search('oslo');
      await Future.delayed(const Duration(milliseconds: 500));

      final state = container.read(searchProvider);
      expect(state, isA<AsyncError>());
      expect((state as AsyncError).error, isA<StateError>());
    });
  });

  group('SearchNotifier.clear', () {
    test('resets state to empty data', () async {
      final service = FakeLocationService(results: [_result('Oslo')]);
      final container = _container(service);
      final notifier = container.read(searchProvider.notifier);

      notifier.search('oslo');
      await Future.delayed(const Duration(milliseconds: 500));
      expect(container.read(searchProvider).value, isNotEmpty);

      notifier.clear();
      expect(container.read(searchProvider).value, isEmpty);
    });
  });
}
