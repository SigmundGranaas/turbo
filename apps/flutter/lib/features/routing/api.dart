/// Public API for the Routing feature — plan on/off-trail routes through
/// the curated tileserver routing API (gateway `/api/route/*`).
///
/// This is the only file other features may import. Models, the HTTP
/// client, repository, providers, and the map layer are the surface;
/// internals stay private. See `lib/context/architecture.context.md`.
library;

export 'data/routing_api_client.dart'
    show RoutingApiClient, RoutingException, RoutingErrorKind;
export 'data/routing_repository.dart' show RoutingRepository;
export 'models/route_models.dart'
    show RouteRequest, RoutePlan, RouteLeg, RoutePreset;
export 'providers/routing_providers.dart'
    show
        routingBaseUrlProvider,
        routingApiClientProvider,
        routingRepositoryProvider,
        routePresetsProvider;
export 'widgets/route_layer.dart' show RoutePolylineLayer;
