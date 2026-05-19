import 'dart:convert';

import 'package:http/http.dart' as http;
import 'package:latlong2/latlong.dart';

import 'package:turbo/core/util/user_agent.dart';
import '../stedsnavn_descriptors.dart';

/// Thin wrapper around Kartverket's `/stedsnavn/v1/punkt` endpoint.
///
/// Returns the best-scored named toponym near [coord] as a
/// [StedsnavnHit], or `null` when no item in the response qualifies
/// (out of Norway, all entries are anonymous, etc.). Network and
/// parse errors collapse to `null` — the orchestrator decides what
/// to do next.
class StedsnavnBackend {
  static const String _host = 'ws.geonorge.no';
  static const String _path = '/stedsnavn/v1/punkt';
  static const Duration _timeout = Duration(seconds: 8);

  final http.Client _client;

  StedsnavnBackend({http.Client? client}) : _client = client ?? http.Client();

  Future<StedsnavnHit?> find(LatLng coord) async {
    try {
      // `navnestatus=hovednavn` drops historic / secondary / disused
      // spellings (the root of the "Close to Unknown" noise).
      //
      // No `filtrer` — the /punkt endpoint's filter syntax differs from
      // /navn (skrivemåte/språk live under `stedsnavn[]`; kommuner/fylker
      // aren't returned at all) and asking for a /navn-shaped fieldset
      // returns HTTP 400 ("Mulig feil i filtreringsparameter"). Payload is
      // already ~8 KB; not worth the fragility.
      final uri = Uri.https(_host, _path, {
        'nord': coord.latitude.toString(),
        'ost': coord.longitude.toString(),
        'koordsys': '4258',
        'radius': '2000',
        'treffPerSide': '25',
        'navnestatus': 'hovednavn',
      });
      final response = await _client.get(uri, headers: const {
        'Accept': 'application/json',
        'User-Agent': kTurboUserAgent,
      }).timeout(_timeout);
      if (response.statusCode != 200) return null;
      final json = jsonDecode(utf8.decode(response.bodyBytes))
          as Map<String, dynamic>;
      final navnList = ((json['navn'] as List?) ?? const [])
          .cast<Map<String, dynamic>>();
      return _pickBest(coord, navnList);
    } catch (_) {
      return null;
    }
  }

  static StedsnavnHit? _pickBest(
      LatLng coord, List<Map<String, dynamic>> items) {
    StedsnavnHit? best;
    for (final item in items) {
      final hit = describeFeature(coord, item);
      if (hit == null) continue;
      if (best == null || hit.score < best.score) {
        best = hit;
      }
    }
    return best;
  }
}
