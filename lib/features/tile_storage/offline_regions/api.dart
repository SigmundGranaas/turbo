/// The public API for the Offline Regions feature.
library;

// Models
export 'models/offline_region.dart';

// Data: provider + notifier
export 'data/offline_regions_notifier.dart'
    show offlineRegionsProvider, OfflineRegionsNotifier;
export 'data/download_orchestrator.dart' show downloadOrchestratorProvider;

// UI
export 'widgets/offline_regions_page.dart';
export 'widgets/region_creation_page.dart';
