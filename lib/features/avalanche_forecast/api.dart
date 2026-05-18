/// Public API for the Avalanche Forecast feature (Varsom / NVE).
library;

export 'models/avalanche_warning.dart'
    show
        AvalancheWarning,
        AvalancheDangerLevel,
        AvalancheProblem,
        shouldShowAvalancheWarning;
export 'data/varsom_service.dart'
    show VarsomService, VarsomServiceException;
export 'data/avalanche_forecast_notifier.dart'
    show
        varsomServiceProvider,
        avalancheForecastProvider,
        AvalancheForecastNotifier;
export 'widgets/avalanche_warning_badge.dart' show AvalancheWarningBadge;
export 'widgets/avalanche_warning_sheet.dart' show AvalancheWarningSheet;
export 'widgets/show_avalanche_warning_sheet.dart'
    show showAvalancheWarningSheet;
