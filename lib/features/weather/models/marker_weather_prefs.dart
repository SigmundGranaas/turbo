import 'package:collection/collection.dart';

import 'weather_metric.dart';

/// User preference of which weather metrics to display for a given marker.
///
/// The marker model itself is untouched; preferences are stored in a separate,
/// local-only table keyed by marker UUID (see
/// `lib/features/weather/data/marker_weather_prefs_store.dart`). A marker with
/// no row gets [defaults].
class MarkerWeatherPrefs {
  static const Set<WeatherMetric> defaultMetrics = {
    WeatherMetric.temperature,
    WeatherMetric.wind,
    WeatherMetric.precipitation,
  };

  final String markerUuid;
  final Set<WeatherMetric> metrics;

  MarkerWeatherPrefs({
    required this.markerUuid,
    required Set<WeatherMetric> metrics,
  }) : metrics = Set.unmodifiable(metrics);

  factory MarkerWeatherPrefs.defaults(String markerUuid) {
    return MarkerWeatherPrefs(
      markerUuid: markerUuid,
      metrics: defaultMetrics,
    );
  }

  MarkerWeatherPrefs copyWith({Set<WeatherMetric>? metrics}) {
    return MarkerWeatherPrefs(
      markerUuid: markerUuid,
      metrics: metrics ?? this.metrics,
    );
  }

  Map<String, dynamic> toJson() => {
        'metrics': metrics.map((m) => m.code).toList(),
      };

  factory MarkerWeatherPrefs.fromJson(
      String markerUuid, Map<String, dynamic> json) {
    final rawCodes = (json['metrics'] as List?) ?? const [];
    final parsed = <WeatherMetric>{};
    for (final raw in rawCodes) {
      if (raw is! String) continue;
      final m = WeatherMetric.byCode(raw);
      if (m != null) parsed.add(m);
    }
    return MarkerWeatherPrefs(markerUuid: markerUuid, metrics: parsed);
  }

  static const _setEquality = SetEquality<WeatherMetric>();

  @override
  bool operator ==(Object other) =>
      identical(this, other) ||
      (other is MarkerWeatherPrefs &&
          other.markerUuid == markerUuid &&
          _setEquality.equals(other.metrics, metrics));

  @override
  int get hashCode => Object.hash(markerUuid, _setEquality.hash(metrics));
}
