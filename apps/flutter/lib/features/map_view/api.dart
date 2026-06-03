/// The public API for the Map View feature.
library;

export 'widgets/main_map_page.dart' show MainMapPage;
export 'widgets/map_base.dart' show MapBase;
export 'widgets/layers/current_location_layer.dart' show CurrentLocationLayer;
export 'widgets/layers/viewport_marker_layer.dart' show ViewportMarkers;
export 'widgets/mode_indicator.dart' show ModeIndicator;
export 'models/map_view_state.dart' show MapViewState;
export 'data/map_view_state_notifier.dart'
    show mapViewStateProvider, MapViewStateNotifier;
export 'models/map_tool.dart'
    show
        MapToolDescriptor,
        MapToolContext,
        MapToolLayersBuilder,
        MapToolOverlayBuilder,
        MapToolTapHandler,
        MapToolInteractionBuilder;
export 'data/map_tool_registry.dart'
    show MapToolRegistry, mapToolRegistryProvider, activeMapToolProvider;
export 'models/map_layer.dart'
    show MapLayerDescriptor, MapLayerContext, MapLayerBuilder;
export 'data/map_layer_registry.dart'
    show MapLayerRegistry, mapLayerRegistryProvider;
export 'models/map_overlay.dart'
    show MapOverlayDescriptor, MapOverlayContext, MapOverlaySlot;
export 'data/map_overlay_registry.dart'
    show MapOverlayRegistry, mapOverlayRegistryProvider;
export 'models/map_entity_action.dart'
    show MapEntityAction, MapEntityActionContext;
export 'data/map_entity_action_registry.dart'
    show MapEntityActionRegistry, mapEntityActionRegistryProvider;
export 'widgets/map_entity_action_bar.dart' show MapEntityActionBar;
export 'models/map_selection.dart' show MapSelection;
export 'data/map_selection_notifier.dart'
    show SelectedMapEntityNotifier, selectedMapEntityProvider;
export 'widgets/map_entity_detail_host.dart' show MapEntityDetailHost;
export 'widgets/coordinate_detail_body.dart' show CoordinateDetailBody;
export 'models/conditions_source.dart' show ConditionsSource;
export 'data/map_conditions_registry.dart'
    show MapConditionsRegistry, mapConditionsRegistryProvider;
