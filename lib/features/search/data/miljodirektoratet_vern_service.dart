import 'dart:convert';

import 'package:http/http.dart' as http;
import 'package:latlong2/latlong.dart';

/// One protected-area hit at a coordinate.
class ProtectedArea {
  /// e.g. "Saltfjellet–Svartisen nasjonalpark".
  final String name;

  /// e.g. "Nasjonalpark", "Naturreservat", "Landskapsvernområde".
  final String kind;

  const ProtectedArea({required this.name, required this.kind});

  bool get isNationalPark => kind.toLowerCase().contains('nasjonalpark');
  bool get isNatureReserve => kind.toLowerCase().contains('naturreservat');
}

/// Queries Miljødirektoratet's Vern (Naturbase) ArcGIS service for
/// national parks, nature reserves, and landscape-protection areas
/// containing a point. The Stedsnavn API doesn't carry these — they
/// live in a completely different dataset — so without this query
/// the pin sheet can never produce "In Jotunheimen" etc.
///
/// Endpoint:
///   https://kart.miljodirektoratet.no/arcgis/rest/services/vern/MapServer/identify
///
/// Out-of-coverage / network errors → `null` list, never throws.
class MiljodirektoratetVernService {
  static const String _host = 'kart.miljodirektoratet.no';
  static const String _path = '/arcgis/rest/services/vern/MapServer/identify';

  final http.Client _client;

  MiljodirektoratetVernService({http.Client? client})
      : _client = client ?? http.Client();

  Future<List<ProtectedArea>> identifyAt(LatLng coord) async {
    try {
      const halfSpan = 0.005; // ~500 m at Nordic latitudes
      final extent = {
        'xmin': coord.longitude - halfSpan,
        'ymin': coord.latitude - halfSpan,
        'xmax': coord.longitude + halfSpan,
        'ymax': coord.latitude + halfSpan,
        'spatialReference': {'wkid': 4326},
      };
      final geometry = {
        'x': coord.longitude,
        'y': coord.latitude,
        'spatialReference': {'wkid': 4326},
      };
      final uri = Uri.https(_host, _path, {
        'geometry': jsonEncode(geometry),
        'geometryType': 'esriGeometryPoint',
        'sr': '4326',
        'tolerance': '1',
        'layers': 'all',
        'mapExtent': jsonEncode(extent),
        'imageDisplay': '400,400,96',
        'returnGeometry': 'false',
        'f': 'json',
      });

      final response = await _client.get(uri, headers: const {
        'Accept': 'application/json',
      });
      if (response.statusCode != 200) return const [];
      final json = jsonDecode(utf8.decode(response.bodyBytes))
          as Map<String, dynamic>;
      final results = (json['results'] as List?) ?? const [];
      final areas = <ProtectedArea>[];
      for (final r in results.whereType<Map<String, dynamic>>()) {
        final layerName = (r['layerName'] as String?)?.trim() ?? '';
        if (layerName.isEmpty) continue;
        final name = _readName(r);
        if (name == null) continue;
        areas.add(ProtectedArea(name: name, kind: layerName));
      }
      return areas;
    } catch (_) {
      return const [];
    }
  }

  /// ArcGIS identify returns the display value either in `value` or
  /// in `attributes[displayFieldName]`. Naturbase commonly uses
  /// "navn" / "Navn" / "områdenavn" / "OMRÅDENAVN".
  static String? _readName(Map<String, dynamic> result) {
    final value = result['value'];
    if (value is String && value.trim().isNotEmpty && value != 'Null') {
      return value.trim();
    }
    final attrs = result['attributes'] as Map<String, dynamic>?;
    if (attrs == null) return null;
    for (final key in const ['navn', 'Navn', 'NAVN', 'områdenavn', 'OMRÅDENAVN', 'NAME']) {
      final v = attrs[key];
      if (v is String && v.trim().isNotEmpty && v != 'Null') {
        return v.trim();
      }
    }
    return null;
  }
}
