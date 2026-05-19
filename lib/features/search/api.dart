export 'data/search_state_provider.dart' show searchProvider, SearchNotifier;
export 'data/location_service.dart'
    show
        LocationSearchResult,
        LocationService,
        LocationDescription,
        LocationQualifier;
export 'data/composite_search_service.dart'
    show compositeSearchServiceProvider, stedsnavnSearchBackendProvider;
export 'data/backends/stedsnavn_search_backend.dart'
    show StedsnavnSearchBackend;
export 'data/stedsnavn_descriptors.dart'
    show LocationMatchTier, StedsnavnHit, describeFeature, readPlaceName;
export 'data/reverse_geocoder.dart'
    show
        ReverseGeocoder,
        reverseGeocoderProvider,
        describeLocationProvider,
        GeoQuery,
        stedsnavnBackendProvider,
        protectedAreaBackendProvider,
        kommuneBackendProvider,
        addressBackendProvider,
        elevationBackendProvider;
export 'data/kartverket_reverse_geocoder.dart' show KartverketReverseGeocoder;
export 'data/backends/stedsnavn_backend.dart' show StedsnavnBackend;
export 'data/backends/protected_area_backend.dart' show ProtectedAreaBackend;
export 'data/backends/kommune_backend.dart' show KommuneBackend;
export 'data/backends/address_backend.dart' show AddressBackend;
export 'data/backends/elevation_backend.dart' show ElevationBackend;
export 'data/trail_search_service.dart'
    show TrailSearchService, trailSearchServiceProvider;
export 'widgets/search_bar_mobile.dart' show MobileSearchBar;
export 'widgets/search_bar_desktop.dart' show DesktopSearchBar;
