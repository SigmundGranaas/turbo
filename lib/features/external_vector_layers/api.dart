/// Public API for the External Vector Layers feature — runtime GeoJSON / WFS
/// layers (trails, weather warnings, protected areas, ...).
library;

export 'models/vector_feature.dart'
    show VectorFeature, VectorGeometryKind;
export 'models/vector_layer_source.dart' show VectorLayerSource;
export 'data/vector_layer_fetcher.dart'
    show
        VectorLayerFetcher,
        VectorLayerFetchException,
        vectorLayerFetcherProvider;
export 'data/vector_layer_cache.dart' show VectorLayerCache;
export 'data/vector_tile_store.dart'
    show VectorTileStore, StoredVectorTile, NoopVectorTileStore;
export 'data/sqlite_vector_tile_store.dart'
    show SqliteVectorTileStore, vectorTileStoreProvider;
export 'data/vector_layer_repository.dart'
    show VectorLayerRepository, vectorLayerRepositoryProvider;
export 'data/vector_layer_notifier.dart'
    show
        viewportVectorFeaturesProvider,
        ViewportVectorFeaturesNotifier;
export 'widgets/vector_data_layer.dart' show VectorDataLayer;
export 'widgets/vector_feature_sheet.dart'
    show VectorFeatureSheet, showVectorFeatureSheet;
export 'data/sources/nasjonal_turbase_source.dart'
    show TrailSubtype, trailVectorSource, trailOverlayIdToSubtype;
export 'data/sources/met_alerts_source.dart' show metAlertsVectorSource;
export 'data/sources/osm_path_source.dart' show osmPathVectorSource;
export 'data/sources/n50_sti_source.dart' show n50StiVectorSource;
export 'widgets/trail_feature_sheet.dart' show TrailFeatureSheet;
export 'widgets/trail_property_decoder.dart'
    show TrailProperties, TrailDifficulty;
