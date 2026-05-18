import 'dart:convert';
import 'package:http/http.dart' as http;
import 'package:latlong2/latlong.dart';
import 'location_service.dart';

class KartverketLocationService extends LocationService {
  static const String baseUrl = 'https://ws.geonorge.no/stedsnavn/v1/navn';
  static const String pointUrl = 'https://ws.geonorge.no/stedsnavn/v1/punkt';
  static const String kommuneUrl =
      'https://ws.geonorge.no/kommuneinfo/v1/punkt';

  /// Allows tests to inject a mock HTTP client. Defaults to a fresh
  /// `http.Client` in production.
  final http.Client _client;

  KartverketLocationService({http.Client? client})
      : _client = client ?? http.Client();

  @override
  Future<List<LocationSearchResult>> findLocationsBy(String name) async {
    if (name.trim().isEmpty) return [];

    try {
      // Manual URI construction to ensure spaces are encoded as %20
      final encodedName = Uri.encodeComponent(name);
      final uri = Uri.parse('$baseUrl?sok=$encodedName*&fuzzy=true&treffPerSide=10');

      final response = await _client.get(uri);

      if (response.statusCode == 200) {
        // Kartverket API returns UTF-8, but sometimes the http package needs help decoding.
        final decodedBody = utf8.decode(response.bodyBytes);
        final json = jsonDecode(decodedBody);
        final List<dynamic> navnList = json['navn'] ?? [];


        return navnList.map((item) => _parseLocation(item)).toList();
      } else {
        return [];
      }
    } catch (e) {
      return [];
    }
  }

  /// Reverse geocodes a coordinate via Kartverket's `/punkt` endpoint.
  /// Returns the closest named feature, or `null` if Kartverket has no name
  /// nearby (common outside Norway). Network or parse errors collapse to
  /// `null` — the caller can fall back to raw coordinates.
  Future<LocationSearchResult?> findLocationByCoord(LatLng coord,
      {double radiusMeters = 500}) async {
    try {
      final uri = Uri.parse('$pointUrl'
          '?nord=${coord.latitude}&ost=${coord.longitude}'
          '&koordsys=4258&radius=${radiusMeters.toInt()}&treffPerSide=1');
      final response = await _client.get(uri);
      if (response.statusCode != 200) return null;
      final decoded = utf8.decode(response.bodyBytes);
      final json = jsonDecode(decoded) as Map<String, dynamic>;
      final navnList = (json['navn'] as List?) ?? const [];
      if (navnList.isEmpty) return null;
      return _parseLocation(navnList.first as Map<String, dynamic>);
    } catch (_) {
      return null;
    }
  }

  /// Lenient reverse geocode for a coord: returns a [LocationDescription]
  /// that's always informative on Norwegian terrain. Strategy:
  ///   1. Pull up to 25 nearby toponyms within ~2 km.
  ///   2. Prefer peak / mountain features within 100 m ("On X"), then
  ///      within 1 km ("Close to X").
  ///   3. Otherwise prefer protected areas ("In Jotunheimen"), then
  ///      settlements / waters / other named features.
  ///   4. If nothing nearby, fall back to the containing kommune via
  ///      `/kommuneinfo/v1/punkt` ("In Lom").
  ///   5. Last resort: `null` so the UI can fall back to raw coords.
  Future<LocationDescription?> describeLocation(LatLng coord) async {
    try {
      final uri = Uri.parse('$pointUrl'
          '?nord=${coord.latitude}&ost=${coord.longitude}'
          '&koordsys=4258&radius=2000&treffPerSide=25');
      final response = await _client.get(uri);
      if (response.statusCode == 200) {
        final decoded = utf8.decode(response.bodyBytes);
        final json = jsonDecode(decoded) as Map<String, dynamic>;
        final navnList = ((json['navn'] as List?) ?? const [])
            .cast<Map<String, dynamic>>();
        final described = _pickBestDescription(coord, navnList);
        if (described != null) return described;
      }
    } catch (_) {
      // Network or parse error — fall through to kommune lookup.
    }
    return _kommuneAt(coord);
  }

  LocationDescription? _pickBestDescription(
      LatLng coord, List<Map<String, dynamic>> items) {
    if (items.isEmpty) return null;
    final distance = const Distance();

    // Score each candidate. Lower score wins.
    Map<String, dynamic>? bestItem;
    double? bestScore;
    LocationQualifier? bestQualifier;
    double? bestDistance;

    for (final item in items) {
      // Kartverket occasionally returns features with no `skrivemåte`
      // (or an empty one) — typically anonymous Gard/Haug entries. The
      // old `_parseLocation` fallback would surface those as the literal
      // string "Unknown", so skip them at the picker and let the next
      // candidate or the kommune lookup take over.
      final name = (item['skrivemåte'] as String?)?.trim();
      if (name == null || name.isEmpty) continue;
      final pt = item['representasjonspunkt'] as Map<String, dynamic>?;
      if (pt == null) continue;
      final lat = (pt['nord'] as num?)?.toDouble();
      final lng = (pt['øst'] as num?)?.toDouble();
      if (lat == null || lng == null) continue;
      final d = distance.as(LengthUnit.Meter, coord, LatLng(lat, lng));
      final kind = (item['navneobjekttype'] as String?)?.toLowerCase() ?? '';
      final (priority, qualifier) = _categorize(kind, d);
      // Skip categories we've decided to discard outright.
      if (priority == null) continue;
      // Composite score: feature priority (×10 km) + distance (m).
      final score = priority * 10000 + d;
      if (bestScore == null || score < bestScore) {
        bestScore = score;
        bestItem = item;
        bestQualifier = qualifier;
        bestDistance = d;
      }
    }

    if (bestItem == null) return null;
    final parsed = _parseLocation(bestItem);
    return LocationDescription(
      title: parsed.title,
      qualifier: bestQualifier,
      secondary: parsed.description,
      distanceMeters: bestDistance,
    );
  }

  /// Returns (priority, qualifier) for a Kartverket feature type at a
  /// given distance. Smaller priority wins. `null` priority means
  /// "ignore this feature entirely".
  (int?, LocationQualifier?) _categorize(String kind, double meters) {
    // Peaks / mountains: best feature for hikers.
    const peakKinds = {
      'fjelltopp',
      'topp',
      'fjell',
      'ås',
      'haug',
      'berg',
      'nut',
      'pigg',
    };
    // Protected areas — only ever phrased as "In X".
    const areaKinds = {
      'nasjonalpark',
      'naturreservat',
      'landskapsvernområde',
      'naturminne',
      'verneområde',
    };
    // Water features.
    const waterKinds = {'innsjø', 'vann', 'elv', 'fjord', 'bekk', 'tjern'};
    // Settlements.
    const settlementKinds = {'tettsted', 'by', 'tettbebyggelse', 'bygd'};
    // Built features.
    const builtKinds = {'gard', 'bruk', 'seter', 'hytte'};

    if (peakKinds.contains(kind)) {
      if (meters <= 100) return (0, LocationQualifier.on);
      if (meters <= 1000) return (1, LocationQualifier.closeTo);
      return (3, LocationQualifier.near);
    }
    if (areaKinds.contains(kind)) {
      return (2, LocationQualifier.inArea);
    }
    if (waterKinds.contains(kind)) {
      if (meters <= 100) return (4, LocationQualifier.atPlace);
      return (5, LocationQualifier.closeTo);
    }
    if (settlementKinds.contains(kind)) {
      if (meters <= 200) return (6, LocationQualifier.inArea);
      return (7, LocationQualifier.near);
    }
    if (builtKinds.contains(kind)) {
      if (meters <= 50) return (8, LocationQualifier.atPlace);
      return (9, LocationQualifier.closeTo);
    }
    // Any other named feature: keep but at low priority.
    if (kind.isEmpty) return (null, null);
    if (meters <= 50) return (10, LocationQualifier.atPlace);
    return (11, LocationQualifier.closeTo);
  }

  /// Fallback: the kommune (municipality) containing [coord]. Returns
  /// e.g. "In Lom". `null` when outside Norway or on network failure.
  Future<LocationDescription?> _kommuneAt(LatLng coord) async {
    try {
      final uri = Uri.parse('$kommuneUrl'
          '?nord=${coord.latitude}&ost=${coord.longitude}'
          '&koordsys=4258');
      final response = await _client.get(uri);
      if (response.statusCode != 200) return null;
      final decoded = utf8.decode(response.bodyBytes);
      final json = jsonDecode(decoded) as Map<String, dynamic>;
      final kommune = json['kommunenavn'] as String?;
      final fylke = json['fylkesnavn'] as String?;
      if (kommune == null || kommune.isEmpty) return null;
      return LocationDescription(
        title: kommune,
        qualifier: LocationQualifier.inArea,
        secondary: fylke,
      );
    } catch (_) {
      return null;
    }
  }

  LocationSearchResult _parseLocation(Map<String, dynamic> item) {
    // Extract coordinates
    final representasjonspunkt = item['representasjonspunkt'] ?? {};
    // API returns coordinates as east/north, which correspond to lng/lat
    final double lat = (representasjonspunkt['nord'] ?? 0.0).toDouble();
    final double lng = (representasjonspunkt['øst'] ?? 0.0).toDouble();

    // Extract location name
    final String name = item['skrivemåte'] ?? 'Unknown';

    // Build description from available data
    final List<String> descriptionParts = [];

    // Add object type
    final String? objectType = item['navneobjekttype'];
    if (objectType != null) {
      descriptionParts.add(objectType);
    }

    // Add kommune (municipality)
    final List<dynamic> kommuner = item['kommuner'] ?? [];
    if (kommuner.isNotEmpty) {
      final String? kommuneName = kommuner.first['kommunenavn'];
      if (kommuneName != null) {
        descriptionParts.add(kommuneName);
      }
    }

    // Add fylke (county)
    final List<dynamic> fylker = item['fylker'] ?? [];
    if (fylker.isNotEmpty) {
      final String? fylkeName = fylker.first['fylkesnavn'];
      if (fylkeName != null) {
        descriptionParts.add(fylkeName);
      }
    }

    final String description = descriptionParts.join(', ');

    // Determine icon based on object type
    String? icon;
    switch (objectType?.toLowerCase()) {
      case 'bruk':
        icon = 'farm';
        break;
      case 'gard':
        icon = 'home';
        break;
      case 'elv':
        icon = 'water';
        break;
      case 'tettbebyggelse':
      case 'by':
        icon = 'city';
        break;
      case 'fjell':
        icon = 'mountain';
        break;
      case 'innsjø':
      case 'vann':
        icon = 'water';
        break;
      default:
        icon = 'place';
    }

    return LocationSearchResult(
      title: name,
      description: description.isNotEmpty ? description : null,
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