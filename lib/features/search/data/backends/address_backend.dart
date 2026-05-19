import 'dart:convert';

import 'package:http/http.dart' as http;
import 'package:latlong2/latlong.dart';

import 'package:turbo/core/util/user_agent.dart';
import '../location_service.dart';

/// Kartverket Adresser `/adresser/v1/punktsok` — returns the closest
/// civic address (street + number + post code + post town) to
/// [coord]. The orchestrator inserts this between the protected-area
/// branch and the kommune fallback so populated rural pins read e.g.
/// "Near Storgården 4, Lom" instead of the bare kommune name.
///
/// Endpoint:
///   https://ws.geonorge.no/adresser/v1/punktsok
///
/// Out-of-coverage / wilderness / network errors → `null`.
class AddressBackend {
  static const String _host = 'ws.geonorge.no';
  static const String _path = '/adresser/v1/punktsok';
  static const Duration _timeout = Duration(seconds: 8);

  final http.Client _client;

  AddressBackend({http.Client? client}) : _client = client ?? http.Client();

  Future<LocationDescription?> nearestAddress(LatLng coord) async {
    try {
      final uri = Uri.https(_host, _path, {
        'lat': coord.latitude.toString(),
        'lon': coord.longitude.toString(),
        'radius': '200',
        'treffPerSide': '1',
      });
      final response = await _client.get(uri, headers: const {
        'Accept': 'application/json',
        'User-Agent': kTurboUserAgent,
      }).timeout(_timeout);
      if (response.statusCode != 200) return null;
      final json = jsonDecode(utf8.decode(response.bodyBytes))
          as Map<String, dynamic>;
      final list = (json['adresser'] as List?) ?? const [];
      if (list.isEmpty) return null;
      final first = list.first;
      if (first is! Map<String, dynamic>) return null;

      final title = (first['adressetekst'] as String?)?.trim();
      if (title == null || title.isEmpty) return null;

      final poststed = (first['poststed'] as String?)?.trim();
      final postnummer = (first['postnummer'] as String?)?.trim();
      final secondary = [
        if (postnummer != null && postnummer.isNotEmpty) postnummer,
        if (poststed != null && poststed.isNotEmpty) poststed,
      ].join(' ');

      return LocationDescription(
        title: title,
        qualifier: LocationQualifier.near,
        secondary: secondary.isEmpty ? null : secondary,
      );
    } catch (_) {
      return null;
    }
  }
}
