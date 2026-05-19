import 'package:latlong2/latlong.dart';

import 'package:turbo/core/api/api_client.dart';
import '../models/backcountry_ski_activity.dart';
import '../models/backcountry_ski_details.dart';

/// HTTP client for /api/activities/backcountry-ski/*. Exchanges typed
/// DTOs; the route is sent as WKT LINESTRING.
class BackcountrySkiApi {
  final ApiClient _client;
  BackcountrySkiApi(this._client);

  Future<String> create({
    required String name,
    String? description,
    required List<LatLng> route,
    required BackcountrySkiDetails details,
  }) async {
    final body = {
      'name': name,
      'description': ?description,
      'routeWkt': _toWkt(route),
      'details': details.toJson(),
    };
    final r = await _client.post('/api/activities/backcountry-ski', data: body);
    if (r.statusCode != 201) {
      throw Exception('Failed to create backcountry ski activity: ${r.statusCode} ${r.data}');
    }
    return (r.data as Map<String, dynamic>)['id'] as String;
  }

  Future<BackcountrySkiActivity> getById(String id) async {
    final r = await _client.get('/api/activities/backcountry-ski/$id');
    if (r.statusCode != 200) {
      throw Exception('Failed to fetch backcountry ski activity $id: ${r.statusCode}');
    }
    return BackcountrySkiActivity.fromJson(r.data as Map<String, dynamic>);
  }

  Future<void> update({
    required String id,
    String? name,
    String? description,
    List<LatLng>? route,
    BackcountrySkiDetails? details,
  }) async {
    final body = <String, dynamic>{
      'name': ?name,
      'description': ?description,
      'routeWkt': ?(route == null ? null : _toWkt(route)),
      'details': ?details?.toJson(),
    };
    final r = await _client.put('/api/activities/backcountry-ski/$id', data: body);
    if (r.statusCode != 204) {
      throw Exception('Failed to update backcountry ski activity $id: ${r.statusCode} ${r.data}');
    }
  }

  Future<void> delete(String id) async {
    final r = await _client.delete('/api/activities/backcountry-ski/$id');
    if (r.statusCode != 204) {
      throw Exception('Failed to delete backcountry ski activity $id: ${r.statusCode}');
    }
  }

  static String _toWkt(List<LatLng> points) {
    if (points.isEmpty) return 'LINESTRING EMPTY';
    final pairs = points.map((p) => '${p.longitude} ${p.latitude}').join(', ');
    return 'LINESTRING($pairs)';
  }
}
