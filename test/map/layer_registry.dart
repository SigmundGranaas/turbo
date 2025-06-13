import 'package:flutter_map/flutter_map.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:turbo/widgets/map/layers/tiles/tile_registry/tile_provider.dart';
import 'package:turbo/widgets/map/layers/tiles/tile_registry/tile_registry.dart';
import 'mock_tile_provider_wrapper.dart';

void main() {
  group('TileRegistry', () {
    late TileRegistry registry;
    late MockTileProviderWrapper globalProvider;
    late MockTileProviderWrapper localProvider;
    late MockTileProviderWrapper overlayProvider;

    setUp(() {
      final container = ProviderContainer();
      registry = container.read(tileRegistryProvider.notifier);

      globalProvider = MockTileProviderWrapper(
        id: 'global-test',
        category: TileCategory.global,
      );

      localProvider = MockTileProviderWrapper(
        id: 'local-test',
        category: TileCategory.local,
      );

      overlayProvider = MockTileProviderWrapper(
        id: 'overlay-test',
        category: TileCategory.overlay,
      );
    });

    group('Provider Registration', () {
      test('Register new providers', () {
        registry.registerProvider(globalProvider);

        expect(
          registry.state.availableProviders,
          contains(globalProvider.id),
        );
      });


      test('Not change selected global when registering non-global provider', () {
        registry.registerProvider(globalProvider);
        final initialGlobalId = registry.state.activeGlobalIds.first;

        registry.registerProvider(overlayProvider);

        expect(registry.state.activeGlobalIds.first, equals(initialGlobalId));
      });

      test('Unregister providers', () {
        registry.registerProvider(globalProvider);
        registry.unregisterProvider(globalProvider.id);

        expect(
          registry.state.availableProviders,
          isNot(contains(globalProvider.id)),
        );
      });

      test('Clear selections when unregistering active provider', () {
        // Register and activate providers
        registry.registerProvider(globalProvider);
        registry.registerProvider(localProvider);
        registry.registerProvider(overlayProvider);

        registry.toggleLocalLayer(localProvider.id);
        registry.toggleOverlay(overlayProvider.id);

        // Unregister active providers
        registry.unregisterProvider(localProvider.id);
        registry.unregisterProvider(overlayProvider.id);

        expect(registry.state.activeLocalIds, isEmpty);
        expect(registry.state.activeOverlayIds, isEmpty);
      });
    });

    group('Layer Selection', () {
      setUp(() {
        registry.registerProvider(globalProvider);
        registry.registerProvider(localProvider);
        registry.registerProvider(overlayProvider);
      });

      test('Change global layer', () {
        final newGlobalProvider = MockTileProviderWrapper(
          id: 'global-test-2',
          category: TileCategory.global,
        );
        registry.registerProvider(newGlobalProvider);

        registry.toggleGlobalLayer(newGlobalProvider.id);

        expect(registry.state.activeGlobalIds.first, equals(newGlobalProvider.id));
      });

      test('Throw when setting non-global provider as global', () {
        expect(
              () => registry.toggleGlobalLayer(localProvider.id),
          throwsArgumentError,
        );
      });

      test('Toggle local layer', () {
        registry.toggleLocalLayer(localProvider.id);
        expect(registry.state.activeLocalIds, contains(localProvider.id));

        registry.toggleLocalLayer(localProvider.id);
        expect(registry.state.activeLocalIds, isNot(contains(localProvider.id)));
      });

      test('Throw when toggling non-local provider as local', () {
        expect(
              () => registry.toggleLocalLayer(globalProvider.id),
          throwsArgumentError,
        );
      });

      test('Toggle overlay', () {
        registry.toggleOverlay(overlayProvider.id);
        expect(registry.state.activeOverlayIds, contains(overlayProvider.id));

        registry.toggleOverlay(overlayProvider.id);
        expect(registry.state.activeOverlayIds, isNot(contains(overlayProvider.id)));
      });

      test('Throw when toggling non-overlay provider as overlay', () {
        expect(
              () => registry.toggleOverlay(globalProvider.id),
          throwsArgumentError,
        );
      });
    });

    group('Active Layers', () {
      setUp(() {
        registry.registerProvider(globalProvider);
        registry.registerProvider(localProvider);
        registry.registerProvider(overlayProvider);
      });

      test('Return layers in correct order', () {
        registry.toggleLocalLayer(localProvider.id);
        registry.toggleOverlay(overlayProvider.id);

        final layers = registry.getActiveLayers();

        expect(layers.length, equals(3));
        expect(layers[0], isA<TileLayer>()); // Global
        expect(layers[1], isA<TileLayer>()); // Local
        expect(layers[2], isA<TileLayer>()); // Overlay
      });

      test('Skip inactive layers', () {
        final layers = registry.getActiveLayers();

        expect(layers.length, equals(1)); // Only global layer
      });

      test('Handle missing global layer', () {
        registry.unregisterProvider(globalProvider.id);
        registry.toggleLocalLayer(localProvider.id);

        final layers = registry.getActiveLayers();

        expect(layers.length, equals(1)); // Only local layer
      });
    });
  });
}
