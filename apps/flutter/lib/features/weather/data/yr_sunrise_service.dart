import 'dart:convert';

import 'package:http/http.dart' as http;
import 'package:http_parser/http_parser.dart';
import 'package:latlong2/latlong.dart';

import 'package:turbo/core/util/user_agent.dart';
import '../models/sun_event.dart';
import 'yr_atmospheric_service.dart' show YrServiceException;

/// Result of a successful Sunrise 3 fetch.
class SunriseForecastResult {
  /// Indexed by `DateTime(year, month, day)` in the **local** time zone the
  /// service was queried with. One entry per requested day.
  final Map<DateTime, SunEvent> sun;
  final Map<DateTime, MoonEvent> moon;
  final DateTime expiresAt;
  final String? lastModified;

  const SunriseForecastResult({
    required this.sun,
    required this.moon,
    required this.expiresAt,
    required this.lastModified,
  });
}

/// Wrapper around MET Norway's `sunrise/3.0/sun` and `sunrise/3.0/moon`
/// endpoints.
///
/// Sunrise 3 is a single-point-per-day API: each request returns events for
/// one local date. The fetcher batches a small number of consecutive days in
/// parallel to populate the weather sheet's day-strip.
class YrSunriseService {
  static const String _host = 'api.met.no';
  static const String _sunPath = '/weatherapi/sunrise/3.0/sun';
  static const String _moonPath = '/weatherapi/sunrise/3.0/moon';

  /// Sun/moon data is cheap to compute and only changes once per day — a
  /// long fallback TTL is fine when the server omits `Expires`.
  static const Duration _defaultCacheTtl = Duration(hours: 6);

  final http.Client _client;

  YrSunriseService({http.Client? client}) : _client = client ?? http.Client();

  /// Fetch sun and moon events for [days] consecutive days starting at
  /// today's local date.
  Future<SunriseForecastResult> fetch(
    LatLng position, {
    int days = 9,
    DateTime? now,
    String? ifModifiedSince,
    SunriseForecastResult? previous,
  }) async {
    final base = now ?? DateTime.now();
    final localStart = DateTime(base.year, base.month, base.day);
    final offset = _offsetString(base.timeZoneOffset);

    final sun = <DateTime, SunEvent>{};
    final moon = <DateTime, MoonEvent>{};
    DateTime? expires;
    String? lastModified;
    var anySuccess = false;

    for (var i = 0; i < days; i++) {
      final day = localStart.add(Duration(days: i));
      final dateStr = _formatDate(day);

      final sunFuture = _fetchOne(
        path: _sunPath,
        position: position,
        date: dateStr,
        offset: offset,
        ifModifiedSince: ifModifiedSince,
      );
      final moonFuture = _fetchOne(
        path: _moonPath,
        position: position,
        date: dateStr,
        offset: offset,
        ifModifiedSince: ifModifiedSince,
      );

      final results = await Future.wait([sunFuture, moonFuture]);
      final sunResp = results[0];
      final moonResp = results[1];

      if (sunResp != null) {
        final event = _parseSun(day, sunResp.body);
        if (event != null) sun[day] = event;
        anySuccess = true;
        expires ??= sunResp.expires;
        lastModified ??= sunResp.lastModified;
      }
      if (moonResp != null) {
        final event = _parseMoon(day, moonResp.body);
        if (event != null) moon[day] = event;
      }
    }

    if (!anySuccess && previous != null) {
      return previous;
    }

    return SunriseForecastResult(
      sun: sun,
      moon: moon,
      expiresAt: expires ?? DateTime.now().toUtc().add(_defaultCacheTtl),
      lastModified: lastModified,
    );
  }

  Future<_RawResponse?> _fetchOne({
    required String path,
    required LatLng position,
    required String date,
    required String offset,
    String? ifModifiedSince,
  }) async {
    final uri = Uri.https(_host, path, {
      'lat': position.latitude.toStringAsFixed(4),
      'lon': position.longitude.toStringAsFixed(4),
      'date': date,
      'offset': offset,
    });
    final request = http.Request('GET', uri)
      ..headers['User-Agent'] = kTurboUserAgent
      ..headers['Accept'] = 'application/json';
    if (ifModifiedSince != null) {
      request.headers['If-Modified-Since'] = ifModifiedSince;
    }

    final streamed = await _client.send(request);
    final response = await http.Response.fromStream(streamed);

    if (response.statusCode == 304) return null;
    if (response.statusCode < 200 || response.statusCode >= 300) {
      // 422 occurs at extreme latitudes for some dates — treat as missing.
      if (response.statusCode == 422 || response.statusCode == 404) return null;
      throw YrServiceException(
        response.statusCode,
        response.body.isEmpty ? 'Empty body' : response.body,
      );
    }
    return _RawResponse(
      body: utf8.decode(response.bodyBytes),
      expires: _readExpires(response.headers),
      lastModified: response.headers['last-modified'],
    );
  }

  static SunEvent? _parseSun(DateTime day, String body) {
    final json = jsonDecode(body) as Map<String, dynamic>;
    final props = json['properties'] as Map<String, dynamic>?;
    if (props == null) return null;
    final sunrise = _readEventTime(props['sunrise']);
    final sunset = _readEventTime(props['sunset']);
    final solarNoon = _readEventTime(props['solarnoon']);
    final solarMidnight = _readEventTime(props['solarmidnight']);

    // MET signals polar conditions by omitting sunrise/sunset together with
    // the solar noon's `visible` field. Sun visible at solar noon and no
    // sunrise/sunset = polar day; sun not visible and no rise/set = polar
    // night.
    final noonVisible = (props['solarnoon']
        as Map<String, dynamic>?)?['visible'] as bool?;
    final polarDay =
        sunrise == null && sunset == null && noonVisible == true;
    final polarNight =
        sunrise == null && sunset == null && noonVisible == false;

    return SunEvent(
      date: day,
      sunrise: sunrise,
      sunset: sunset,
      solarNoon: solarNoon,
      solarMidnight: solarMidnight,
      polarDay: polarDay,
      polarNight: polarNight,
    );
  }

  static MoonEvent? _parseMoon(DateTime day, String body) {
    final json = jsonDecode(body) as Map<String, dynamic>;
    final props = json['properties'] as Map<String, dynamic>?;
    if (props == null) return null;
    final raw = props['moonphase'];
    return MoonEvent(
      date: day,
      moonrise: _readEventTime(props['moonrise']),
      moonset: _readEventTime(props['moonset']),
      phaseDegrees: raw is num ? raw.toDouble() : null,
    );
  }

  static DateTime? _readEventTime(Object? block) {
    if (block is! Map<String, dynamic>) return null;
    final time = block['time'] as String?;
    if (time == null) return null;
    return DateTime.parse(time);
  }

  static String _formatDate(DateTime d) {
    final m = d.month.toString().padLeft(2, '0');
    final day = d.day.toString().padLeft(2, '0');
    return '${d.year}-$m-$day';
  }

  static String _offsetString(Duration offset) {
    final sign = offset.isNegative ? '-' : '+';
    final abs = offset.abs();
    final h = abs.inHours.toString().padLeft(2, '0');
    final m = (abs.inMinutes % 60).toString().padLeft(2, '0');
    return '$sign$h:$m';
  }

  static DateTime _readExpires(Map<String, String> headers) {
    final raw = headers['expires'];
    if (raw != null) {
      try {
        return parseHttpDate(raw).toUtc();
      } on FormatException {
        // Fall through to default.
      }
    }
    return DateTime.now().toUtc().add(_defaultCacheTtl);
  }
}

class _RawResponse {
  final String body;
  final DateTime expires;
  final String? lastModified;
  const _RawResponse({
    required this.body,
    required this.expires,
    required this.lastModified,
  });
}
