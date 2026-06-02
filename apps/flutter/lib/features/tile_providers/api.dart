/// The public API for the Tile Providers feature.
library;

// Models
export 'models/tile_provider_config.dart';
export 'models/tile_registry_state.dart';

// Built-in provider configs that other features need to reference.
export 'data/providers/osm_tiles.dart' show OsmConfig;

// State: the registry notifier and its provider.
export 'data/tile_registry.dart' show tileRegistryProvider, TileRegistry;

// Derived providers for consumers (UI layer pickers, the map page).
export 'data/tile_layer_providers.dart'
    show
        activeTileLayersProvider,
        mapProjectionProvider,
        globalLayersProvider,
        localLayersProvider,
        overlayLayersProvider,
        offlineLayersProvider;

// Custom (user-defined) tile providers.
export 'data/custom_provider_store.dart'
    show customProviderStoreProvider, CustomProviderStore;
export 'models/custom_tile_provider.dart'
    show CustomTileProvider, CustomTileProviderConfig;
export 'widgets/add_custom_map_page.dart'
    show AddCustomMapPage, pushAddCustomMapPage;
