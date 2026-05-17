import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:latlong2/latlong.dart';

import '../models/weather_forecast.dart';
import '../models/weather_metric.dart';
import 'weather_fetcher.dart';
import 'yr_atmospheric_service.dart';
import 'yr_ocean_service.dart';

/// Value-typed key for the forecast cache.
///
/// Position is rounded to 4 decimals (MET TOS) so neighbouring queries share a
/// cache entry. Sources are sorted into a canonical ordered representation so
/// `{atmospheric, marine}` and `{marine, atmospheric}` hash the same.
class WeatherRequest {
  final LatLng position;
  final Set<WeatherMetricSource> sources;

  WeatherRequest({
    required LatLng position,
    required Set<WeatherMetricSource> sources,
  })  : position = LatLng(
          double.parse(position.latitude.toStringAsFixed(4)),
          double.parse(position.longitude.toStringAsFixed(4)),
        ),
        sources = Set.unmodifiable(sources);

  String get _sourceKey {
    final names = sources.map((s) => s.name).toList()..sort();
    return names.join(',');
  }

  @override
  bool operator ==(Object other) =>
      identical(this, other) ||
      (other is WeatherRequest &&
          other.position.latitude == position.latitude &&
          other.position.longitude == position.longitude &&
          other._sourceKey == _sourceKey);

  @override
  int get hashCode =>
      Object.hash(position.latitude, position.longitude, _sourceKey);
}

final yrAtmosphericServiceProvider =
    Provider<YrAtmosphericService>((ref) => YrAtmosphericService());

final yrOceanServiceProvider =
    Provider<YrOceanService>((ref) => YrOceanService());

final weatherFetcherProvider = Provider<WeatherFetcher>((ref) {
  return WeatherFetcher(
    atmospheric: ref.watch(yrAtmosphericServiceProvider),
    ocean: ref.watch(yrOceanServiceProvider),
  );
});

/// Async family notifier holding one merged [WeatherForecast] per request
/// shape. Honors MET's `Expires` semantics: a read within the freshness
/// window returns the cached forecast without hitting the network; an
/// expired read revalidates via `If-Modified-Since` (handled by the fetcher).
final weatherForecastProvider = AsyncNotifierProvider.family<
    WeatherForecastNotifier, WeatherForecast, WeatherRequest>(
  WeatherForecastNotifier.new,
);

class WeatherForecastNotifier
    extends AsyncNotifier<WeatherForecast> {
  WeatherForecastNotifier(this.request);

  final WeatherRequest request;
  WeatherForecast? _cached;

  @override
  Future<WeatherForecast> build() async {
    final cached = _cached;
    if (cached != null && cached.isFresh) return cached;
    final fetched = await ref
        .read(weatherFetcherProvider)
        .fetch(request.position, request.sources, previous: cached);
    _cached = fetched;
    return fetched;
  }

  /// Force a re-fetch, bypassing the cache.
  Future<void> refresh() async {
    _cached = null;
    ref.invalidateSelf();
    await future;
  }
}
