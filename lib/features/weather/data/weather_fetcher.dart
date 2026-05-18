import 'dart:async';
import 'dart:developer' as developer;

import 'package:latlong2/latlong.dart';

import '../models/weather_forecast.dart';
import 'yr_atmospheric_service.dart';
import 'yr_ocean_service.dart';

/// Combines the MET atmospheric + ocean endpoints into a single
/// [WeatherForecast].
///
/// Both endpoints are always tried in parallel. Atmospheric failures bubble
/// up (we have no useful UI without it). Marine failures are swallowed and
/// translate to an empty marine list — the marker is simply inland or out of
/// MET's marine coverage, which is the common case.
class WeatherFetcher {
  final YrAtmosphericService atmospheric;
  final YrOceanService ocean;

  WeatherFetcher({required this.atmospheric, required this.ocean});

  Future<WeatherForecast> fetch(
    LatLng position, {
    WeatherForecast? previous,
  }) async {
    final atmFuture = atmospheric.fetch(
      position,
      ifModifiedSince: previous?.atmosphericLastModified,
      previous: previous == null
          ? null
          : AtmosphericForecastResult(
              points: previous.atmospheric,
              expiresAt: previous.atmosphericExpiresAt,
              lastModified: previous.atmosphericLastModified,
            ),
    );
    final marineFuture = _safeMarineFetch(position, previous);

    final atmResult = await atmFuture;
    final marineResult = await marineFuture;

    return WeatherForecast(
      position: position,
      fetchedAt: DateTime.now().toUtc(),
      atmosphericExpiresAt: atmResult.expiresAt,
      marineExpiresAt: marineResult?.expiresAt,
      atmosphericLastModified: atmResult.lastModified,
      marineLastModified: marineResult?.lastModified,
      atmospheric: atmResult.points,
      marine: marineResult?.points ?? const [],
    );
  }

  Future<MarineForecastResult?> _safeMarineFetch(
      LatLng position, WeatherForecast? previous) async {
    try {
      return await ocean.fetch(
        position,
        ifModifiedSince: previous?.marineLastModified,
        previous: previous == null || previous.marine.isEmpty
            ? null
            : MarineForecastResult(
                points: previous.marine,
                expiresAt:
                    previous.marineExpiresAt ?? DateTime.now().toUtc(),
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
