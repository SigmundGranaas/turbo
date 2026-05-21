import 'package:latlong2/latlong.dart';

import 'package:turbo/core/api/api_client.dart';
import '../models/freediving_activity.dart';
import '../models/freediving_conditions_report.dart';
import '../models/freediving_details.dart';

class FreedivingApi {
  final ApiClient _client;
  FreedivingApi(this._client);

  Future<String> create({
    required String name, String? description,
    required LatLng position, required FreedivingDetails details,
  }) async {
    final body = {
      'name': name, 'description': ?description,
      'longitude': position.longitude, 'latitude': position.latitude,
      'details': details.toJson(),
    };
    final r = await _client.post('/api/activities/freediving', data: body);
    if (r.statusCode != 201) throw Exception('Failed to create freediving activity: ${r.statusCode} ${r.data}');
    return (r.data as Map<String, dynamic>)['id'] as String;
  }

  Future<FreedivingActivity> getById(String id) async {
    final r = await _client.get('/api/activities/freediving/$id');
    if (r.statusCode != 200) throw Exception('Failed to fetch freediving activity $id: ${r.statusCode}');
    return FreedivingActivity.fromJson(r.data as Map<String, dynamic>);
  }

  Future<void> update({
    required String id, String? name, String? description,
    LatLng? position, FreedivingDetails? details,
  }) async {
    final body = <String, dynamic>{
      'name': ?name, 'description': ?description,
      'longitude': ?position?.longitude, 'latitude': ?position?.latitude,
      'details': ?details?.toJson(),
    };
    final r = await _client.put('/api/activities/freediving/$id', data: body);
    if (r.statusCode != 204) throw Exception('Failed to update freediving activity $id: ${r.statusCode} ${r.data}');
  }

  Future<void> delete(String id) async {
    final r = await _client.delete('/api/activities/freediving/$id');
    if (r.statusCode != 204) throw Exception('Failed to delete freediving activity $id: ${r.statusCode}');
  }

  Future<FreedivingConditionsReport> getConditions(String id, {DateTime? at}) async {
    final query = <String, dynamic>{};
    if (at != null) query['at'] = at.toUtc().toIso8601String();
    final r = await _client.get('/api/activities/freediving/$id/conditions', queryParameters: query);
    if (r.statusCode != 200) throw Exception('Failed to fetch conditions for $id: ${r.statusCode}');
    return FreedivingConditionsReport.fromJson(r.data as Map<String, dynamic>);
  }
}
