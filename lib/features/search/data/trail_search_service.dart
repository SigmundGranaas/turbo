import 'dart:convert';

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:http/http.dart' as http;
import 'package:latlong2/latlong.dart';

import 'package:turbo/core/util/user_agent.dart';
import 'location_service.dart';

/// Search source backed by Geonorge's "Nasjonal turbase" WFS feed.
///
/// We call the WFS GetFeature endpoint with a CQL `LIKE` filter against the
/// trail `navn` field. Returns up to 10 matches as [LocationSearchResult]s
/// keyed by the first vertex of the trail (good enough for "zoom to result"
/// behaviour).
class TrailSearchService extends LocationService {
  static const String _host = 'wfs.geonorge.no';
  static const String _path = '/skwms1/wfs.friluftsruter2';

  final http.Client _client;
  TrailSearchService({http.Client? client}) : _client = client ?? http.Client();

  @override
  Future<List<LocationSearchResult>> findLocationsBy(String name) async {
    final trimmed = name.trim();
    if (trimmed.isEmpty) return const [];

    final uri = Uri.https(_host, _path, {
      'SERVICE': 'WFS',
      'VERSION': '2.0.0',
      'REQUEST': 'GetFeature',
      'TYPENAMES': 'app:Fotrute',
      'OUTPUTFORMAT': 'application/json',
      'SRSNAME': 'EPSG:4326',
      'COUNT': '10',
      'CQL_FILTER': "navn ILIKE '%${_escape(trimmed)}%'",
    });

    try {
      final response = await _client.get(uri, headers: {
        'User-Agent': kTurboUserAgent,
        'Accept': 'application/json',
      });
      if (response.statusCode != 200) return const [];
      return _parse(utf8.decode(response.bodyBytes));
    } catch (_) {
      return const [];
    }
  }

  static List<LocationSearchResult> _parse(String body) {
    final json = jsonDecode(body);
    if (json is! Map<String, dynamic>) return const [];
    final features = json['features'];
    if (features is! List) return const [];

    final out = <LocationSearchResult>[];
    for (final raw in features) {
      if (raw is! Map<String, dynamic>) continue;
      final props = raw['properties'] is Map<String, dynamic>
          ? raw['properties'] as Map<String, dynamic>
          : const {};
      final name = (props['navn'] ?? '').toString();
      if (name.isEmpty) continue;
      final position = _firstPoint(raw['geometry']);
      if (position == null) continue;

      final description = <String>[
        if (props['rutenummer'] != null) props['rutenummer'].toString(),
        if (props['merkemetode'] != null) props['merkemetode'].toString(),
      ].join(' · ');

      out.add(LocationSearchResult(
        title: name,
        description: description.isEmpty ? null : description,
        position: position,
        icon: 'vandring',
        source: 'turbase',
        metadata: {
          'rutenummer': props['rutenummer'],
          'merkemetode': props['merkemetode'],
        },
      ));
    }
    return out;
  }

  static LatLng? _firstPoint(Object? geometry) {
    if (geometry is! Map<String, dynamic>) return null;
    final coords = geometry['coordinates'];
    if (coords is! List || coords.isEmpty) return null;
    final type = geometry['type'];
    Object? firstPair;
    if (type == 'LineString') {
      firstPair = coords.first;
    } else if (type == 'MultiLineString') {
      final first = coords.first;
      if (first is List && first.isNotEmpty) firstPair = first.first;
    }
    if (firstPair is! List || firstPair.length < 2) return null;
    return LatLng(
      (firstPair[1] as num).toDouble(),
      (firstPair[0] as num).toDouble(),
    );
  }

  static String _escape(String input) => input.replaceAll("'", "''");
}

final trailSearchServiceProvider =
    Provider<TrailSearchService>((_) => TrailSearchService());
