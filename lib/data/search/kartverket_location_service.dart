import 'dart:convert';
import 'package:flutter/foundation.dart';
import 'package:http/http.dart' as http;
import 'package:latlong2/latlong.dart';
import 'package:map_app/data/search/location_service.dart';
import 'api/kartverket_stedsnavn.dart';

class KartverketLocationService extends LocationService {
  @override
  Future<List<LocationSearchResult>> findLocationsBy(String name) async {
    if (name.trim().isEmpty) {
      return [];
    }
    try {
      final response = await http.get(KartverketApiRequest(navn: name).uri());
      if (response.statusCode == 200) {
        // Use utf8.decode to handle potential special characters in the response.
        final data = KartverketApiResponse.fromJson(json.decode(utf8.decode(response.bodyBytes)));
        return convertResponsesToLocationResults(data.navn);
      }
      if (kDebugMode) {
        print("Kartverket search failed with status: ${response.statusCode}, body: ${response.body}");
      }
      return [];
    } catch (e) {
      if (kDebugMode) {
        print('Error fetching suggestions from Kartverket: $e');
      }
      return [];
    }
  }

  List<LocationSearchResult> convertResponsesToLocationResults(List<PlaceName> results) {
    return results.map(_convertToResult).toList();
  }

  LocationSearchResult _convertToResult(PlaceName data) {
    final name = data.skrivemate;
    final type = data.navneobjekttype;
    final municipality = data.kommuner.isNotEmpty ? "${data.kommuner[0].kommunenavn} kommune" : "Ukjent";
    final description = '$type i $municipality';
    final icon = _findIconFromType(type);
    final latlng = LatLng(data.representasjonspunkt.nord, data.representasjonspunkt.ost);

    return LocationSearchResult(
        title: name, description: description, position: latlng, icon: icon);
  }

  String? _findIconFromType(String type) {
    return switch (type.toLowerCase()) {
      'fjelltopp' => 'Fjell',
      'fjell' => 'Fjell',
      'skog' => 'Skog',
      'park' => '',
      _ => null,
    };
  }
}