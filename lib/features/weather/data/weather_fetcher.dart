import 'dart:async';
import 'dart:developer' as developer;

import 'package:latlong2/latlong.dart';

import '../models/weather_forecast.dart';
import '../models/weather_metric.dart';
import 'yr_atmospheric_service.dart';
import 'yr_ocean_service.dart';

/// Orchestrates atmospheric + marine fetches into a single [WeatherForecast].
///
/// Decides which endpoints to hit based on the requested sources:
///  * `atmospheric` only -> just the atmospheric service.
///  * `marine` only      -> just the ocean service.
///  * both               -> both in parallel.
///
/// Failure handling is asymmetric: the atmospheric side surfaces exceptions
/// to the caller (we have no useful UI without atmospheric data), while a
/// marine failure is swallowed and treated as "no marine data" — matching
/// the user-confirmed UX where marine rows silently disappear outside
/// MET's coverage area.
class WeatherFetcher {
  final YrAtmosphericService atmospheric;
  final YrOceanService ocean;

  WeatherFetcher({required this.atmospheric, required this.ocean});

  Future<WeatherForecast> fetch(
    LatLng position,
    Set<WeatherMetricSource> sources, {
    WeatherForecast? previous,
  }) async {
    final wantAtm = sources.contains(WeatherMetricSource.atmospheric);
    final wantMarine = sources.contains(WeatherMetricSource.marine);

    final atmFuture = wantAtm
        ? atmospheric.fetch(
            position,
            ifModifiedSince: previous?.atmosphericLastModified,
            previous: previous == null
                ? null
                : AtmosphericForecastResult(
                    points: previous.atmospheric,
                    expiresAt: previous.atmosphericExpiresAt,
                    lastModified: previous.atmosphericLastModified,
                  ),
          )
        : null;
    final marineFuture = wantMarine
        ? _safeOceanFetch(position, previous)
        : null;

    final atmResult = atmFuture == null ? null : await atmFuture;
    final marineResult = marineFuture == null ? null : await marineFuture;

    return WeatherForecast(
      position: position,
      fetchedAt: DateTime.now().toUtc(),
      atmosphericExpiresAt:
          atmResult?.expiresAt ?? DateTime.now().toUtc(),
      marineExpiresAt: marineResult?.expiresAt,
      atmosphericLastModified: atmResult?.lastModified,
      marineLastModified: marineResult?.lastModified,
      atmospheric: atmResult?.points ?? const [],
      marine: marineResult?.points ?? const [],
    );
  }

  Future<MarineForecastResult?> _safeOceanFetch(
      LatLng position, WeatherForecast? previous) async {
    try {
      return await ocean.fetch(
        position,
        ifModifiedSince: previous?.marineLastModified,
        previous: previous == null || previous.marine.isEmpty
            ? null
            : MarineForecastResult(
                points: previous.marine,
                expiresAt: previous.marineExpiresAt ??
                    DateTime.now().toUtc(),
                lastModified: previous.marineLastModified,
              ),
      );
    } catch (e, st) {
      developer.log(
        'Marine fetch failed; hiding marine rows',
        name: 'WeatherFetcher',
        error: e,
        stackTrace: st,
      );
      return null;
    }
  }
}
