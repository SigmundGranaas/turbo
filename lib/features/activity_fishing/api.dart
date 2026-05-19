// Public façade for the fishing activity kind feature.

export 'descriptor.dart' show fishingActivityKindDescriptor;
export 'models/fishing_activity.dart' show FishingActivity;
export 'models/fishing_details.dart'
    show FishingDetails, WaterKind, ShoreOrBoat, TargetSpecies, DepthSample, PreferredConditions;
export 'data/fishing_repository.dart'
    show fishingRepositoryProvider, FishingRepository, fishingActivityProvider, fishingApiProvider;
export 'data/fishing_api.dart' show FishingApi;
export 'widgets/fishing_create_screen.dart' show FishingCreateScreen;
export 'widgets/fishing_detail_sheet.dart' show FishingDetailSheet;
