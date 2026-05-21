import 'dart:async';
import 'dart:developer' as developer;

import 'package:latlong2/latlong.dart';

import '../models/weather_forecast.dart';
import 'metalerts_service.dart';
import 'yr_atmospheric_service.dart';
import 'yr_ocean_service.dart';
import 'yr_sunrise_service.dart';

/// Combines the MET atmospheric + ocean + sunrise + alerts endpoints into a
/// single [WeatherForecast].
///
/// Atmospheric is the only required side — failures bubble up because the UI
/// has nothing useful to show without it. Marine, sun, moon, and alert
/// failures are swallowed and degrade silently.
class WeatherFetcher {
  final YrAtmosphericService atmospheric;
  final YrOceanService ocean;
  final YrSunriseService sunrise;
  final MetAlertsService alerts;

  WeatherFetcher({
    required this.atmospheric,
    required this.ocean,
    required this.sunrise,
    required this.alerts,
  });

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
    final sunFuture = _safeSunFetch(position);
    final alertsFuture = _safeAlertsFetch(position);

    final atmResult = await atmFuture;
    final marineResult = await marineFuture;
    final sunResult = await sunFuture;
    final alertsResult = await alertsFuture;

    return WeatherForecast(
      position: position,
      fetchedAt: DateTime.now().toUtc(),
      atmosphericExpiresAt: atmResult.expiresAt,
      marineExpiresAt: marineResult?.expiresAt,
      atmosphericLastModified: atmResult.lastModified,
      marineLastModified: marineResult?.lastModified,
      atmospheric: atmResult.points,
      marine: marineResult?.points ?? const [],
      sun: sunResult?.sun ?? const {},
      moon: sunResult?.moon ?? const {},
      alerts: alertsResult?.alerts ?? const [],
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

  Future<SunriseForecastResult?> _safeSunFetch(LatLng position) async {
    try {
      return await sunrise.fetch(position);
    } catch (e, st) {
      developer.log(
        'Sunrise fetch failed; hiding sun strip',
        name: 'WeatherFetcher',
        error: e,
        stackTrace: st,
      );
      return null;
    }
  }

  Future<MetAlertsResult?> _safeAlertsFetch(LatLng position) async {
    try {
      return await alerts.currentAtPoint(position);
    } catch (e, st) {
      developer.log(
        'MetAlerts fetch failed; hiding alert banner',
        name: 'WeatherFetcher',
        error: e,
        stackTrace: st,
      );
      return null;
    }
  }
}
