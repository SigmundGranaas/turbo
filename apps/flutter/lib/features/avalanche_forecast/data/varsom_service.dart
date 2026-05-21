import 'dart:convert';

import 'package:http/http.dart' as http;
import 'package:latlong2/latlong.dart';

import 'package:turbo/core/util/user_agent.dart';
import '../models/avalanche_warning.dart';

class VarsomServiceException implements Exception {
  final int statusCode;
  final String message;
  const VarsomServiceException(this.statusCode, this.message);
  @override
  String toString() => 'VarsomServiceException($statusCode): $message';
}

/// Wrapper around the Varsom (NVE) regional snow-avalanche forecast API.
///
/// Endpoint: `api01.nve.no/hydrology/forecast/avalanche/v6.2.1`
///
/// The "AvalancheWarningByCoordinates/Detail" endpoint accepts WGS84
/// lat/lon and returns one warning per requested day. Outside the
/// Norwegian mountain coverage area the response is an empty array, which
/// this service surfaces as a `null` warning.
class VarsomService {
  static const String _host = 'api01.nve.no';
  static const String _basePath =
      '/hydrology/forecast/avalanche/v6.2.1/api/AvalancheWarningByCoordinates/Detail';

  final http.Client _client;
  final String _language;

  /// [language] must be `'1'` for Norwegian or `'2'` for English (Varsom
  /// API uses numeric codes).
  VarsomService({http.Client? client, String language = '2'})
      : _client = client ?? http.Client(),
        _language = language;

  /// Fetch today's warning for [position]. Returns `null` when Varsom has
  /// no coverage for the coordinate (point lies outside Norwegian mountain
  /// regions) or no warning has been issued for the day.
  Future<AvalancheWarning?> forToday(LatLng position, {DateTime? now}) async {
    final day = now ?? DateTime.now();
    final date = _formatDate(DateTime(day.year, day.month, day.day));
    final uri = Uri.https(
      _host,
      '$_basePath/${position.latitude.toStringAsFixed(4)}/'
      '${position.longitude.toStringAsFixed(4)}/$_language/$date/$date',
    );

    final response = await _client.get(uri, headers: {
      'User-Agent': kTurboUserAgent,
      'Accept': 'application/json',
    });

    if (response.statusCode == 204) return null;
    if (response.statusCode < 200 || response.statusCode >= 300) {
      throw VarsomServiceException(
        response.statusCode,
        response.body.isEmpty ? 'Empty body' : response.body,
      );
    }

    final body = utf8.decode(response.bodyBytes);
    final decoded = jsonDecode(body);
    if (decoded is! List || decoded.isEmpty) return null;

    final entry = decoded.first;
    if (entry is! Map<String, dynamic>) return null;
    return _parse(entry);
  }

  static AvalancheWarning? _parse(Map<String, dynamic> json) {
    final levelRaw = json['DangerLevel'];
    int? levelInt;
    if (levelRaw is int) {
      levelInt = levelRaw;
    } else if (levelRaw is String) {
      levelInt = int.tryParse(levelRaw);
    }
    final level = AvalancheDangerLevel.fromNumeric(levelInt);
    if (level == null) return null;

    final regionId = (json['RegionId'] is int)
        ? json['RegionId'] as int
        : int.tryParse('${json['RegionId']}') ?? 0;
    final regionName = (json['RegionName'] ?? '').toString();
    final validRaw = json['ValidFrom'] ?? json['DateValid'];
    DateTime validDate;
    if (validRaw is String) {
      validDate = DateTime.tryParse(validRaw) ?? DateTime.now();
    } else {
      validDate = DateTime.now();
    }
    final problems = <AvalancheProblem>[];
    final probJson = json['AvalancheProblems'];
    if (probJson is List) {
      for (final p in probJson) {
        if (p is! Map<String, dynamic>) continue;
        problems.add(AvalancheProblem(
          typeName: (p['AvalancheProblemTypeName'] ?? p['Problem']) as String?,
          sensitivity: p['AvalTriggerSimpleName'] as String?,
          distribution: p['AvalPropagationName'] as String?,
          size: p['DestructiveSizeExtName'] as String?,
        ));
      }
    }

    return AvalancheWarning(
      regionId: regionId,
      regionName: regionName,
      validDate: validDate,
      dangerLevel: level,
      mainText: json['MainText'] as String?,
      avalancheDanger: json['AvalancheDanger'] as String?,
      problems: problems,
    );
  }

  static String _formatDate(DateTime d) {
    final m = d.month.toString().padLeft(2, '0');
    final day = d.day.toString().padLeft(2, '0');
    return '${d.year}-$m-$day';
  }
}
