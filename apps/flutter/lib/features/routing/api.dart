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
export 'models/route_geo_path.dart' show RoutePlanGeoPath;
export 'providers/routing_providers.dart'
    show
        routingBaseUrlProvider,
        routingApiClientProvider,
        routingRepositoryProvider,
        routePresetsProvider;
export 'data/route_planning_notifier.dart'
    show RoutePlanningNotifier, routePlanningProvider;
export 'data/route_planning_state.dart' show RoutePlanningState;
export 'widgets/route_layer.dart' show RoutePolylineLayer;
export 'widgets/route_planning_tool.dart'
    show routePlanningTool, routePlanningToolId;
