// Public façade for the Activities shell feature. The shell intentionally
// does NOT re-export kind-specific types — each kind has its own
// `features/activity_<kind>/api.dart` for that. Cross-feature imports must
// go through this file.

export 'models/activity_geometry.dart'
    show ActivityGeometry, ActivityGeometryKind;
export 'models/activity_summary.dart' show ActivitySummary, ActivitySummaryTombstone;
export 'models/activity_kind_descriptor.dart' show ActivityKindDescriptor;
export 'data/activity_kind_registry.dart'
    show ActivityKindRegistry, activityKindRegistryProvider;
export 'data/activity_summaries_repository.dart'
    show activitySummariesRepositoryProvider, ActivitySummariesRepository,
        activitySummariesApiProvider;
export 'data/activity_summaries_api.dart'
    show ActivitySummariesApi, ActivitySummariesResponse, ActivitySummariesDelta;
export 'widgets/activity_create_picker.dart' show ActivityCreatePicker;
export 'widgets/activities_map_layer.dart' show ActivitiesMapLayer;
