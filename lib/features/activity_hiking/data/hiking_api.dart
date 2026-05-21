import 'package:latlong2/latlong.dart';

import 'package:turbo/core/api/api_client.dart';
import '../models/hiking_activity.dart';
import '../models/hiking_conditions_report.dart';
import '../models/hiking_details.dart';

class HikingApi {
  final ApiClient _client;
  HikingApi(this._client);

  Future<String> create({
    required String name,
    String? description,
    required List<LatLng> route,
    required HikingDetails details,
  }) async {
    final body = {
      'name': name,
      'description': ?description,
      'routeWkt': _wkt(route),
      'details': details.toJson(),
    };
    final r = await _client.post('/api/activities/hiking', data: body);
    if (r.statusCode != 201) throw Exception('Failed to create hiking activity: ${r.statusCode} ${r.data}');
    return (r.data as Map<String, dynamic>)['id'] as String;
  }

  Future<HikingActivity> getById(String id) async {
    final r = await _client.get('/api/activities/hiking/$id');
    if (r.statusCode != 200) throw Exception('Failed to fetch hiking activity $id: ${r.statusCode}');
    return HikingActivity.fromJson(r.data as Map<String, dynamic>);
  }

  Future<void> update({
    required String id,
    String? name,
    String? description,
    List<LatLng>? route,
    HikingDetails? details,
  }) async {
    final body = <String, dynamic>{
      'name': ?name,
      'description': ?description,
      'routeWkt': ?(route == null ? null : _wkt(route)),
      'details': ?details?.toJson(),
    };
    final r = await _client.put('/api/activities/hiking/$id', data: body);
    if (r.statusCode != 204) throw Exception('Failed to update hiking activity $id: ${r.statusCode} ${r.data}');
  }

  Future<void> delete(String id) async {
    final r = await _client.delete('/api/activities/hiking/$id');
    if (r.statusCode != 204) throw Exception('Failed to delete hiking activity $id: ${r.statusCode}');
  }

  Future<HikingConditionsReport> getConditions(String id, {DateTime? at}) async {
    final query = <String, dynamic>{};
    if (at != null) query['at'] = at.toUtc().toIso8601String();
    final r = await _client.get('/api/activities/hiking/$id/conditions', queryParameters: query);
    if (r.statusCode != 200) {
      throw Exception('Failed to fetch conditions for $id: ${r.statusCode}');
    }
    return HikingConditionsReport.fromJson(r.data as Map<String, dynamic>);
  }

  static String _wkt(List<LatLng> points) {
    if (points.isEmpty) return 'LINESTRING EMPTY';
    return 'LINESTRING(${points.map((p) => '${p.longitude} ${p.latitude}').join(', ')})';
  }
}
