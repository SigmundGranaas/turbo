import 'package:latlong2/latlong.dart';

import 'package:turbo/core/api/api_client.dart';
import '../models/xc_ski_activity.dart';
import '../models/xc_ski_conditions_report.dart';
import '../models/xc_ski_details.dart';

class XcSkiApi {
  final ApiClient _client;
  XcSkiApi(this._client);

  Future<String> create({
    required String name, String? description,
    required List<LatLng> route, required XcSkiDetails details,
  }) async {
    final body = {
      'name': name, 'description': ?description,
      'routeWkt': _wkt(route), 'details': details.toJson(),
    };
    final r = await _client.post('/api/activities/xc-ski', data: body);
    if (r.statusCode != 201) throw Exception('Failed to create xc ski activity: ${r.statusCode} ${r.data}');
    return (r.data as Map<String, dynamic>)['id'] as String;
  }

  Future<XcSkiActivity> getById(String id) async {
    final r = await _client.get('/api/activities/xc-ski/$id');
    if (r.statusCode != 200) throw Exception('Failed to fetch xc ski activity $id: ${r.statusCode}');
    return XcSkiActivity.fromJson(r.data as Map<String, dynamic>);
  }

  Future<void> update({
    required String id, String? name, String? description,
    List<LatLng>? route, XcSkiDetails? details,
  }) async {
    final body = <String, dynamic>{
      'name': ?name, 'description': ?description,
      'routeWkt': ?(route == null ? null : _wkt(route)),
      'details': ?details?.toJson(),
    };
    final r = await _client.put('/api/activities/xc-ski/$id', data: body);
    if (r.statusCode != 204) throw Exception('Failed to update xc ski activity $id: ${r.statusCode} ${r.data}');
  }

  Future<void> delete(String id) async {
    final r = await _client.delete('/api/activities/xc-ski/$id');
    if (r.statusCode != 204) throw Exception('Failed to delete xc ski activity $id: ${r.statusCode}');
  }

  Future<XcSkiConditionsReport> getConditions(String id, {DateTime? at}) async {
    final query = <String, dynamic>{};
    if (at != null) query['at'] = at.toUtc().toIso8601String();
    final r = await _client.get('/api/activities/xc-ski/$id/conditions', queryParameters: query);
    if (r.statusCode != 200) throw Exception('Failed to fetch conditions for $id: ${r.statusCode}');
    return XcSkiConditionsReport.fromJson(r.data as Map<String, dynamic>);
  }

  static String _wkt(List<LatLng> points) {
    if (points.isEmpty) return 'LINESTRING EMPTY';
    return 'LINESTRING(${points.map((p) => '${p.longitude} ${p.latitude}').join(', ')})';
  }
}
