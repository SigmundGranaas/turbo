import 'dart:convert';

import 'package:http/http.dart' as http;
import 'package:latlong2/latlong.dart';

import 'package:turbo/core/util/user_agent.dart';
import '../location_service.dart';

/// Wraps Miljødirektoratet's Vern (Naturbase) ArcGIS Identify service
/// — the authoritative source for national parks, nature reserves,
/// and landscape-protected areas (`verneområder`). The Stedsnavn API
/// does not carry these polygons, so without this backend the
/// orchestrator cannot produce "In Jotunheimen nasjonalpark".
///
/// Endpoint:
///   https://kart.miljodirektoratet.no/arcgis/rest/services/vern/MapServer/identify
///
/// Out-of-coverage / network errors → `null`, never throws.
class ProtectedAreaBackend {
  static const String _host = 'kart.miljodirektoratet.no';
  static const String _basePath = '/arcgis/rest/services/vern/MapServer';
  static const String _identifyPath = '$_basePath/identify';
  static const Duration _timeout = Duration(seconds: 8);

  /// Default scope used until the MapServer index has been probed (and
  /// when probing fails). Layer 1 is empirically the class-split polygons
  /// — `all` alone also returns boundary lines and proposed areas, which
  /// produce false / duplicate hits.
  static const String _fallbackScope = 'all:1';

  final http.Client _client;

  /// Lazily-resolved `layers=` scope. Cached per instance so the probe
  /// runs at most once per app launch; subsequent identify calls reuse
  /// the resolved value without an extra round-trip.
  Future<String>? _layerScopeFuture;

  ProtectedAreaBackend({http.Client? client})
      : _client = client ?? http.Client();

  Future<LocationDescription?> identifyAt(LatLng coord) async {
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
      final scope = await _resolveLayerScope();
      final uri = Uri.https(_host, _identifyPath, {
        'geometry': jsonEncode(geometry),
        'geometryType': 'esriGeometryPoint',
        'sr': '4326',
        'tolerance': '1',
        // Scope auto-discovered from the MapServer index, with `all:1`
        // as the fallback when the index is unreachable. See
        // [_discoverLayerScope] for the polygon-layer heuristic.
        'layers': scope,
        'mapExtent': jsonEncode(extent),
        'imageDisplay': '400,400,96',
        'returnGeometry': 'false',
        'f': 'json',
      });
      final response = await _client.get(uri, headers: const {
        'Accept': 'application/json',
        'User-Agent': kTurboUserAgent,
      }).timeout(_timeout);
      if (response.statusCode != 200) return null;
      final json = jsonDecode(utf8.decode(response.bodyBytes))
          as Map<String, dynamic>;
      final results = (json['results'] as List?) ?? const [];
      final areas = <_Area>[];
      for (final r in results.whereType<Map<String, dynamic>>()) {
        final name = _readName(r);
        if (name == null) continue;
        final kind = _readKind(r);
        if (kind == null) continue;
        areas.add(_Area(name, kind));
      }
      return _pickArea(areas);
    } catch (_) {
      return null;
    }
  }

  Future<String> _resolveLayerScope() =>
      _layerScopeFuture ??= _discoverLayerScope();

  /// Probes `MapServer?f=json` to find the polygon "protection-class"
  /// layer by inspecting `geometryType` and `name`, so a future
  /// Naturbase renumbering doesn't silently break the orchestrator.
  /// Returns `all:1` on any failure — that's the layer id we've always
  /// shipped and it stays correct for the current schema.
  Future<String> _discoverLayerScope() async {
    try {
      final indexUri = Uri.https(_host, _basePath, {'f': 'json'});
      final response = await _client.get(indexUri, headers: const {
        'Accept': 'application/json',
        'User-Agent': kTurboUserAgent,
      }).timeout(_timeout);
      if (response.statusCode != 200) return _fallbackScope;
      final json = jsonDecode(utf8.decode(response.bodyBytes))
          as Map<String, dynamic>;
      final layers = (json['layers'] as List?) ?? const [];
      final id = _pickPolygonLayerId(layers);
      return id == null ? _fallbackScope : 'all:$id';
    } catch (_) {
      return _fallbackScope;
    }
  }

  /// Scores polygon layers in the MapServer index. Prefers names
  /// containing "klasse" / "vern" (the class-split protection polygons)
  /// and excludes proposed-area / boundary-line variants. Ties broken
  /// by lower layer id. Returns `null` if no plausible layer exists,
  /// in which case the caller uses the [_fallbackScope].
  static int? _pickPolygonLayerId(List layers) {
    int? best;
    int bestScore = -1;
    for (final raw in layers.whereType<Map<String, dynamic>>()) {
      if (raw['geometryType'] != 'esriGeometryPolygon') continue;
      final name = (raw['name'] as String? ?? '').toLowerCase();
      // Exclude "Foreslåtte verneområder" (proposed) and any
      // boundary / line polygon variants. Naturbase mirrors line
      // representations of polygons under the same service.
      if (name.contains('foresl') ||
          name.contains('plan') ||
          name.contains('linje') ||
          name.contains('grense')) {
        continue;
      }
      final id = (raw['id'] as num?)?.toInt();
      if (id == null) continue;
      var score = 0;
      if (name.contains('klasse')) score += 10;
      if (name.contains('vern')) score += 5;
      // Bias toward the lowest id when names tie.
      score += (100 - id).clamp(0, 100);
      if (score > bestScore) {
        bestScore = score;
        best = id;
      }
    }
    return best;
  }

  /// National park → nature reserve → landscape-protected area →
  /// anything else returned by the service.
  static LocationDescription? _pickArea(List<_Area> areas) {
    if (areas.isEmpty) return null;
    _Area? choice;
    for (final a in areas) {
      if (a.isNationalPark) {
        choice = a;
        break;
      }
    }
    choice ??= areas.firstWhere(
      (a) => a.isNatureReserve,
      orElse: () => areas.first,
    );
    return LocationDescription(
      title: choice.name,
      qualifier: LocationQualifier.inArea,
      secondary: choice.kind,
    );
  }

  /// ArcGIS identify returns the display value either in `value` or
  /// `attributes[displayFieldName]`. Naturbase commonly uses
  /// "navn" / "Navn" / "områdenavn" / "OMRÅDENAVN".
  static String? _readName(Map<String, dynamic> result) {
    final value = result['value'];
    if (value is String && value.trim().isNotEmpty && value != 'Null') {
      return value.trim();
    }
    final attrs = result['attributes'] as Map<String, dynamic>?;
    if (attrs == null) return null;
    for (final key in const [
      'navn',
      'Navn',
      'NAVN',
      'områdenavn',
      'OMRÅDENAVN',
      'omradenavn',
      'NAME',
    ]) {
      final v = attrs[key];
      if (v is String && v.trim().isNotEmpty && v != 'Null') {
        return v.trim();
      }
    }
    return null;
  }

  /// `verneform` is Naturbase's canonical protection-class label
  /// ("Nasjonalpark", "Naturreservat", "Landskapsvernområde", …).
  /// Falls back to ArcGIS `layerName` only when `verneform` is
  /// missing — the latter is presentation-layer text and can drift.
  static String? _readKind(Map<String, dynamic> result) {
    final attrs = result['attributes'] as Map<String, dynamic>?;
    if (attrs != null) {
      for (final key in const ['verneform', 'Verneform', 'VERNEFORM']) {
        final v = attrs[key];
        if (v is String && v.trim().isNotEmpty && v != 'Null') {
          return v.trim();
        }
      }
    }
    final layerName = (result['layerName'] as String?)?.trim() ?? '';
    return layerName.isEmpty ? null : layerName;
  }
}

class _Area {
  final String name;
  final String kind;
  const _Area(this.name, this.kind);
  bool get isNationalPark => kind.toLowerCase().contains('nasjonalpark');
  bool get isNatureReserve => kind.toLowerCase().contains('naturreservat');
}
