import 'dart:convert';

import 'package:http/http.dart' as http;
import 'package:latlong2/latlong.dart';

import 'package:turbo/core/util/user_agent.dart';

/// Kartverket Høydedata `/hoydedata/v1/punkt` — returns the DTM
/// elevation (metres above sea level) at [coord]. Used by the
/// orchestrator as an enrichment side-call: "Galdhøpiggen, 2469 m".
///
/// Endpoint:
///   https://ws.geonorge.no/hoydedata/v1/punkt
///
/// Out-of-coverage (outside Norway, ocean tiles) or network errors →
/// `null`, never throws.
class ElevationBackend {
  static const String _host = 'ws.geonorge.no';
  static const String _path = '/hoydedata/v1/punkt';
  static const Duration _timeout = Duration(seconds: 8);

  final http.Client _client;

  ElevationBackend({http.Client? client}) : _client = client ?? http.Client();

  Future<double?> elevationAt(LatLng coord) async {
    try {
      final uri = Uri.https(_host, _path, {
        'nord': coord.latitude.toString(),
        'ost': coord.longitude.toString(),
        'koordsys': '4258',
        'geojson': 'false',
      });
      final response = await _client.get(uri, headers: const {
        'Accept': 'application/json',
        'User-Agent': kTurboUserAgent,
      }).timeout(_timeout);
      if (response.statusCode != 200) return null;
      final json = jsonDecode(utf8.decode(response.bodyBytes))
          as Map<String, dynamic>;
      final punkter = (json['punkter'] as List?) ?? const [];
      if (punkter.isEmpty) return null;
      final first = punkter.first;
      if (first is! Map<String, dynamic>) return null;
      final z = first['z'];
      if (z is num) return z.toDouble();
      return null;
    } catch (_) {
      return null;
    }
  }
}
