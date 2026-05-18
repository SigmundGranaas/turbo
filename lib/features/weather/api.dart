/// Public API for the Weather feature.
///
/// All consumers outside `lib/features/weather/` import this file only.
library;

export 'models/weather_symbol.dart' show WeatherSymbol;
export 'models/weather_forecast.dart'
    show WeatherForecast, AtmosphericPoint, MarinePoint, DailySummary;
export 'models/tide_forecast.dart'
    show TideForecast, TideExtremum, TideKind;
export 'data/yr_atmospheric_service.dart'
    show YrAtmosphericService, AtmosphericForecastResult, YrServiceException;
export 'data/yr_ocean_service.dart'
    show YrOceanService, MarineForecastResult;
export 'data/kartverket_tide_service.dart' show KartverketTideService;
export 'data/weather_fetcher.dart' show WeatherFetcher;
export 'data/weather_notifier.dart'
    show
        weatherForecastProvider,
        weatherFetcherProvider,
        yrAtmosphericServiceProvider,
        yrOceanServiceProvider,
        WeatherForecastNotifier;
export 'data/tide_notifier.dart'
    show
        tideForecastProvider,
        kartverketTideServiceProvider,
        TideForecastNotifier;
export 'widgets/weather_summary_row.dart' show WeatherSummaryRow;
export 'widgets/weather_detail_sheet.dart'
    show WeatherDetailSheet, showWeatherDetailSheet;
