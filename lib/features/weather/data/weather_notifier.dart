import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:latlong2/latlong.dart';

import '../models/weather_forecast.dart';
import 'weather_fetcher.dart';
import 'yr_atmospheric_service.dart';
import 'yr_ocean_service.dart';

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

/// Async family notifier holding one merged [WeatherForecast] per coordinate.
///
/// Honors MET's `Expires` via an in-memory cache: a read within the freshness
/// window returns the cached forecast without hitting the network; an expired
/// read revalidates via `If-Modified-Since` (handled inside the fetcher).
/// Coordinates are rounded to 4 decimals inside the service URL (MET TOS);
/// this notifier keys on the exact [LatLng] you pass in, which for saved
/// markers is stable.
final weatherForecastProvider = AsyncNotifierProvider.family<
    WeatherForecastNotifier, WeatherForecast, LatLng>(
  WeatherForecastNotifier.new,
);

class WeatherForecastNotifier extends AsyncNotifier<WeatherForecast> {
  WeatherForecastNotifier(this.position);

  final LatLng position;
  WeatherForecast? _cached;

  @override
  Future<WeatherForecast> build() async {
    final cached = _cached;
    if (cached != null && cached.isFresh) return cached;
    final fetched = await ref
        .read(weatherFetcherProvider)
        .fetch(position, previous: cached);
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
