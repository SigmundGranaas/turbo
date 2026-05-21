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
        // Tightened from 2 km → 1 km. The picker's class-4 ranges max
        // out near 1 km anyway, and a smaller radius cuts noisy hits
        // from anonymous Gard/Haug clusters at the periphery.
        'radius': '1000',
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
    // Kartverket regularly returns several entries for the same toponym
    // (e.g. "Galdhøpiggen" classified as both `Fjell` and `Fjelltopp`).
    // Dedupe on lowercased title BEFORE scoring so the duplicates
    // collapse to a single, best-typed candidate — the tier ordering
    // in `categorizeFeature` does the right thing (Fjelltopp beats
    // Fjell at the same distance because the matched range is tighter).
    final bestPerTitle = <String, StedsnavnHit>{};
    for (final item in items) {
      final hit = describeFeature(coord, item);
      if (hit == null) continue;
      final key = hit.description.title.toLowerCase();
      final prior = bestPerTitle[key];
      if (prior == null || hit.score < prior.score) {
        bestPerTitle[key] = hit;
      }
    }
    StedsnavnHit? best;
    for (final hit in bestPerTitle.values) {
      if (best == null || hit.score < best.score) best = hit;
    }
    return best;
  }
}
