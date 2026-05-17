/// Which MET endpoint a metric is sourced from.
enum WeatherMetricSource { atmospheric, marine }

/// One of the data points a user can opt into for a saved marker.
///
/// Source classification controls whether the marine endpoint is hit at all
/// when fetching a forecast: a marker that hasn't opted into any marine metric
/// never probes the ocean API.
enum WeatherMetric {
  temperature('temperature', WeatherMetricSource.atmospheric),
  precipitation('precipitation', WeatherMetricSource.atmospheric),
  snow('snow', WeatherMetricSource.atmospheric),
  wind('wind', WeatherMetricSource.atmospheric),
  humidity('humidity', WeatherMetricSource.atmospheric),
  pressure('pressure', WeatherMetricSource.atmospheric),
  cloudCover('cloudCover', WeatherMetricSource.atmospheric),
  uvIndex('uvIndex', WeatherMetricSource.atmospheric),
  waveHeight('waveHeight', WeatherMetricSource.marine),
  waveDirection('waveDirection', WeatherMetricSource.marine),
  waterTemperature('waterTemperature', WeatherMetricSource.marine);

  const WeatherMetric(this.code, this.source);

  /// Stable string code used for persistence (JSON). Distinct from `name` to
  /// keep the wire format independent of Dart-level renames.
  final String code;
  final WeatherMetricSource source;

  static WeatherMetric? byCode(String code) {
    for (final m in WeatherMetric.values) {
      if (m.code == code) return m;
    }
    return null;
  }

  /// The set of endpoint sources required to fetch the given metrics.
  static Set<WeatherMetricSource> sourcesFor(Set<WeatherMetric> metrics) {
    return metrics.map((m) => m.source).toSet();
  }
}
