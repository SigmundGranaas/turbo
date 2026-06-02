/// The public API for the Map View feature.
library;

export 'widgets/main_map_page.dart' show MainMapPage;
export 'widgets/map_base.dart' show MapBase;
export 'models/map_view_state.dart' show MapViewState;
export 'data/map_view_state_notifier.dart'
    show mapViewStateProvider, MapViewStateNotifier;
export 'data/norway_utm33_crs.dart'
    show
        MapProjection,
        norwayUtm33Crs,
        crsForProjection,
        convertZoomBetweenProjections;
