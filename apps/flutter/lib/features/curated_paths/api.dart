/// Public API for the Curated Paths feature — vector tiles + GeoJSON
/// served by the self-hosted Turbo tileserver (apps/tileserver).
///
/// The map layer (MvtDataLayer) and the per-resource Riverpod sources
/// (curatedSourcesByIdProvider, individual MvtLayerSource providers)
/// are the only cross-feature surface. Internal models, fetchers,
/// repositories, and decoders stay private to keep the architecture
/// boundary clean — see lib/context/architecture.context.md.
library;

export 'models/mvt_layer_source.dart' show MvtLayerSource, MvtFeatureSheetBuilder;
export 'providers/curated_path_providers.dart'
    show
        tileserverBaseUrlProvider,
        curatedHikingSourceProvider,
        curatedSkiTracksSourceProvider,
        curatedForestRoadsSourceProvider,
        curatedCyclingRoutesSourceProvider,
        curatedSourcesByIdProvider;
export 'widgets/mvt_data_layer.dart' show MvtDataLayer;
