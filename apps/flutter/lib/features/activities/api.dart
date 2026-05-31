// Public façade for the Activities shell feature. The shell intentionally
// does NOT re-export kind-specific types — each kind has its own
// `features/activity_<kind>/api.dart` for that. Cross-feature imports must
// go through this file.

export 'models/activity_geometry.dart'
    show ActivityGeometry, ActivityGeometryKind;
export 'models/activity_summary.dart' show ActivitySummary, ActivitySummaryTombstone;
export 'models/activity_kind_descriptor.dart' show ActivityKindDescriptor;
export 'models/driver_keys.dart' show DriverKeys;
export 'models/activity_analysis.dart'
    show
        ActivityAnalysis,
        AnalysisDriver,
        ForecastBand,
        ForecastSample,
        AnalysisTimeWindow,
        AnalysisWarning,
        AnalysisProvenance,
        AnalysisSourceHit,
        ScoreConfidence,
        WindowQuality,
        WarningSeverity;
export 'data/activity_kind_registry.dart'
    show ActivityKindRegistry, activityKindRegistryProvider;
export 'data/activity_summaries_repository.dart'
    show activitySummariesRepositoryProvider, ActivitySummariesRepository,
        activitySummariesApiProvider;
export 'data/activity_summaries_api.dart'
    show ActivitySummariesApi, ActivitySummariesResponse, ActivitySummariesDelta;
export 'data/conditions_cache.dart'
    show fetchConditionsCached, fetchActivityCached, fetchAnalysisCached;
export 'widgets/activity_create_picker.dart' show ActivityCreatePicker;
export 'widgets/activities_map_layer.dart' show ActivitiesMapLayer;
export 'widgets/route_drawing_screen.dart'
    show RouteDrawingScreen, routeDistanceMeters;
export 'widgets/activity_conditions_map.dart' show ActivityConditionsMap;
export 'widgets/analysis/analysis_surface.dart' show AnalysisSurface;
export 'widgets/detail/activity_detail_chassis.dart'
    show ActivityDetailChassis, ActivityAction;
export 'widgets/detail/activity_verdict.dart'
    show ActivityVerdict, VerdictTone, VerdictState;
export 'widgets/detail/activity_stat_strip.dart'
    show ActivityStatStrip, StatItem;
export 'widgets/detail/activity_weather_panel.dart'
    show
        ActivityWeatherPanel,
        WeatherLoadingState,
        WeatherSummary,
        WeatherMetric,
        WeatherHour;
export 'widgets/detail/activity_weather_panel_from_analysis.dart'
    show
        ActivityWeatherPanelFromAnalysis,
        WeatherDriverConfig,
        WeatherMetrics;
export 'widgets/detail/activity_map_preview_from_analysis.dart'
    show ActivityMapPreviewFromAnalysis;
export 'widgets/detail/activity_loading_hint.dart'
    show ActivityLoadingHint;
export 'widgets/detail/activity_delete_dialog.dart'
    show showActivityDeleteDialog;
export 'widgets/detail/condition_palette.dart'
    show ConditionPalette;
export 'widgets/detail/activity_chip_row.dart'
    show ActivityChipRow, ActivityModuleCard;
export 'widgets/observation/observation_draft.dart' show ObservationDraft;
export 'widgets/observation/observation_form.dart'
    show ActivityObservationForm, ObservationSubmit, postObservation;
