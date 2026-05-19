import 'dart:convert';

import 'package:http/http.dart' as http;
import 'package:latlong2/latlong.dart';

import 'package:turbo/core/util/user_agent.dart';
import '../location_service.dart';

/// Kartverket's Kommuneinfo `/punkt` endpoint — returns the
/// municipality (and fylke) containing [coord]. Used as the final
/// fallback when no other source produced a contextual label.
///
/// Endpoint:
///   https://ws.geonorge.no/kommuneinfo/v1/punkt
///
/// Out-of-coverage / network errors → `null`.
class KommuneBackend {
  static const String _host = 'ws.geonorge.no';
  static const String _path = '/kommuneinfo/v1/punkt';
  static const Duration _timeout = Duration(seconds: 8);

  final http.Client _client;

  KommuneBackend({http.Client? client}) : _client = client ?? http.Client();

  Future<LocationDescription?> lookup(LatLng coord) async {
    try {
      final uri = Uri.https(_host, _path, {
        'nord': coord.latitude.toString(),
        'ost': coord.longitude.toString(),
        'koordsys': '4258',
      });
      final response = await _client.get(uri, headers: const {
        'Accept': 'application/json',
        'User-Agent': kTurboUserAgent,
      }).timeout(_timeout);
      if (response.statusCode != 200) return null;
      final json = jsonDecode(utf8.decode(response.bodyBytes))
          as Map<String, dynamic>;
      final kommune = (json['kommunenavn'] as String?)?.trim();
      final fylke = (json['fylkesnavn'] as String?)?.trim();
      if (kommune == null || kommune.isEmpty) return null;
      return LocationDescription(
        title: kommune,
        // No qualifier: "In Bodø" reads correctly for a tettsted but
        // weird for the kommune (which can be a giant rural polygon).
        // The bare kommune name is enough context as a final fallback;
        // genuine containment ("In Lom (town)" / "In Saltfjellet–
        // Svartisen nasjonalpark") still keeps the prefix via the
        // Stedsnavn and Vern backends respectively.
        qualifier: null,
        secondary: (fylke == null || fylke.isEmpty) ? null : fylke,
      );
    } catch (_) {
      return null;
    }
  }
}
