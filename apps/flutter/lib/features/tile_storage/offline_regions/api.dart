/// The public API for the Offline Regions feature.
library;

// Models
export 'models/offline_region.dart';

// Data: provider + notifier
export 'data/offline_regions_notifier.dart'
    show offlineRegionsProvider, OfflineRegionsNotifier;
export 'data/download_orchestrator.dart' show downloadOrchestratorProvider;
export 'data/hidden_downloads.dart'
    show hiddenDownloadsProvider, HiddenDownloadsNotifier;
export 'data/route_corridor.dart' show corridorBounds;

// UI
export 'widgets/download_progress_toolbar.dart' show DownloadProgressToolbar;
export 'widgets/download_details_sheet.dart' show DownloadDetailsSheet;
export 'widgets/offline_regions_page.dart';
export 'widgets/region_select_tool.dart'
    show regionSelectTool, regionSelectToolId;
