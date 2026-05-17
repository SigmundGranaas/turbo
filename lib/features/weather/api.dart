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
