import 'dart:convert';
import 'package:http/http.dart' as http;
import 'package:latlong2/latlong.dart';
import 'location_service.dart';

class KartverketLocationService extends LocationService {
  static const String baseUrl = 'https://ws.geonorge.no/stedsnavn/v1/navn';

  @override
  Future<List<LocationSearchResult>> findLocationsBy(String name) async {
    if (name.trim().isEmpty) return [];

    try {
      // Manual URI construction to ensure spaces are encoded as %20
      final encodedName = Uri.encodeComponent(name);
      final uri = Uri.parse('$baseUrl?sok=$encodedName*&fuzzy=true&treffPerSide=10');

      final response = await http.get(uri);

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