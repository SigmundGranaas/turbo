import 'dart:convert';

import 'package:http/http.dart' as http;
import 'package:latlong2/latlong.dart';
import 'package:turbo/core/util/user_agent.dart';

import '../location_service.dart';
import '../stedsnavn_descriptors.dart';

/// Kartverket Stedsnavn forward-search backend — the typed-query side of
/// the API. Sits alongside the reverse-geocode `StedsnavnBackend`
/// (`/stedsnavn/v1/punkt`); this one hits `/stedsnavn/v1/navn` for the
/// search bar.
///
/// Implements [LocationService] so the composite search service can
/// treat marker / path / Kartverket sources uniformly.
class StedsnavnSearchBackend extends LocationService {
  static const String _baseUrl = 'https://ws.geonorge.no/stedsnavn/v1/navn';
  static const Duration _timeout = Duration(seconds: 8);

  final http.Client _client;

  StedsnavnSearchBackend({http.Client? client})
      : _client = client ?? http.Client();

  @override
  Future<List<LocationSearchResult>> findLocationsBy(String name) async {
    if (name.trim().isEmpty) return [];

    try {
      // Manual URI construction to ensure spaces are encoded as %20.
      final encodedName = Uri.encodeComponent(name);
      final uri = Uri.parse(
          '$_baseUrl?sok=$encodedName*&fuzzy=true&treffPerSide=10');
      final response = await _client.get(uri, headers: const {
        'Accept': 'application/json',
        'User-Agent': kTurboUserAgent,
      }).timeout(_timeout);
      if (response.statusCode != 200) return [];
      final decoded = utf8.decode(response.bodyBytes);
      final json = jsonDecode(decoded) as Map<String, dynamic>;
      final navnList = ((json['navn'] as List?) ?? const [])
          .cast<Map<String, dynamic>>();
      // Drop anonymous entries — they used to surface as "Unknown".
      // [readPlaceName] (shared with the picker) catches missing /
      // empty / "Ukjent" skrivemåte.
      final results = <LocationSearchResult>[];
      for (final item in navnList) {
        final parsed = _parseLocation(item);
        if (parsed.title.isEmpty) continue;
        results.add(parsed);
      }
      return results;
    } catch (_) {
      return [];
    }
  }

  LocationSearchResult _parseLocation(Map<String, dynamic> item) {
    final pt = item['representasjonspunkt'] as Map<String, dynamic>? ?? {};
    final lat = (pt['nord'] as num?)?.toDouble() ?? 0.0;
    final lng = (pt['øst'] as num?)?.toDouble() ?? 0.0;
    final name = readPlaceName(item) ?? '';

    final descriptionParts = <String>[];
    final objectType = item['navneobjekttype'] as String?;
    if (objectType != null) descriptionParts.add(objectType);

    final kommuner = (item['kommuner'] as List?) ?? const [];
    if (kommuner.isNotEmpty) {
      final k = (kommuner.first as Map?)?['kommunenavn'] as String?;
      if (k != null) descriptionParts.add(k);
    }
    final fylker = (item['fylker'] as List?) ?? const [];
    if (fylker.isNotEmpty) {
      final f = (fylker.first as Map?)?['fylkesnavn'] as String?;
      if (f != null) descriptionParts.add(f);
    }
    final description = descriptionParts.join(', ');

    final icon = switch (objectType?.toLowerCase()) {
      'bruk' => 'farm',
      'gard' => 'home',
      'elv' => 'water',
      'tettbebyggelse' || 'by' => 'city',
      'fjell' => 'mountain',
      'innsjø' || 'vann' => 'water',
      _ => 'place',
    };

    return LocationSearchResult(
      title: name,
      description: description.isEmpty ? null : description,
      position: LatLng(lat, lng),
      icon: icon,
      source: 'kartverket',
      metadata: {
        'stedsnummer': item['stedsnummer'],
        'status': item['stedstatus'],
        'språk': item['språk'],
      },
    );
  }
}
