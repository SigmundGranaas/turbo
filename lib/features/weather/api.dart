/// Public API for the Weather feature.
///
/// All consumers outside `lib/features/weather/` import this file only.
library;

export 'models/weather_metric.dart' show WeatherMetric, WeatherMetricSource;
export 'models/weather_symbol.dart' show WeatherSymbol;
export 'models/marker_weather_prefs.dart' show MarkerWeatherPrefs;
export 'models/weather_forecast.dart'
    show WeatherForecast, AtmosphericPoint, MarinePoint, DailySummary;
export 'data/yr_atmospheric_service.dart'
    show YrAtmosphericService, AtmosphericForecastResult, YrServiceException;
export 'data/yr_ocean_service.dart'
    show YrOceanService, MarineForecastResult;
export 'data/weather_fetcher.dart' show WeatherFetcher;
export 'data/marker_weather_prefs_store.dart'
    show MarkerWeatherPrefsStore, markerWeatherPrefsTable;
export 'data/marker_weather_prefs_notifier.dart'
    show
        markerWeatherPrefsProvider,
        markerWeatherPrefsStoreProvider,
        MarkerWeatherPrefsNotifier;
export 'data/weather_notifier.dart'
    show
        WeatherRequest,
        weatherForecastProvider,
        weatherFetcherProvider,
        yrAtmosphericServiceProvider,
        yrOceanServiceProvider,
        WeatherForecastNotifier;
export 'widgets/weather_metrics_sheet.dart' show WeatherMetricsSheet;
export 'widgets/weather_section.dart' show WeatherSection;
