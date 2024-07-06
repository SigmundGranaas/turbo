import 'dart:convert';
import 'package:http/http.dart' as http;
import 'package:latlong2/latlong.dart';
import 'package:map_app/data/search/location_service.dart';
import 'api/kartverket_stedsnavn.dart';

class KartverketLocationService extends LocationService {

  @override
  Future<List<LocationSearchResult>> findLocationsBy(String name) async {
    try {
      final response = await http.get(KartverketApiRequest(navn: name).uri());
      if (response.statusCode == 200) {
        final data = KartverketApiResponse.fromJson(json.decode(response.body));
        return convertResponsesToLocationResults(data.navn);
      }
      return Future.error(Error());
    } catch (e) {
      return Future.error(Error());
      //print('Error fetching suggestions: $e');
    }
  }

  List<LocationSearchResult> convertResponsesToLocationResults(List<PlaceName> results) {
    return results.map(_convertToResult).toList();
  }

  LocationSearchResult _convertToResult(PlaceName data) {
    final name = data.skrivemate;
    final type = data.navneobjekttype;
    final municipality = "${data.kommuner[0].kommunenavn} kommune";
    final description = '$type i $municipality';
    final icon = _findIconFromType(type);
    final latlng = LatLng(data.representasjonspunkt.ost, data.representasjonspunkt.nord);

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
