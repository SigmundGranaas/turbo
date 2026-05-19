import 'package:latlong2/latlong.dart';

import 'package:turbo/core/api/api_client.dart';
import '../models/fishing_activity.dart';
import '../models/fishing_details.dart';

/// HTTP client for the typed fishing kind endpoints under
/// `/api/activities/fishing/*`. Each call exchanges typed DTOs — there
/// is no generic activity write endpoint and no JSONB on the wire.
class FishingApi {
  final ApiClient _client;

  FishingApi(this._client);

  Future<String> create({
    required String name,
    String? description,
    required LatLng position,
    required FishingDetails details,
  }) async {
    final body = {
      'name': name,
      'description': ?description,
      'longitude': position.longitude,
      'latitude': position.latitude,
      'details': details.toJson(),
    };
    final r = await _client.post('/api/activities/fishing', data: body);
    if (r.statusCode != 201) {
      throw Exception('Failed to create fishing activity: ${r.statusCode} ${r.data}');
    }
    return (r.data as Map<String, dynamic>)['id'] as String;
  }

  Future<FishingActivity> getById(String id) async {
    final r = await _client.get('/api/activities/fishing/$id');
    if (r.statusCode != 200) {
      throw Exception('Failed to fetch fishing activity $id: ${r.statusCode}');
    }
    return FishingActivity.fromJson(r.data as Map<String, dynamic>);
  }

  Future<void> update({
    required String id,
    String? name,
    String? description,
    LatLng? position,
    FishingDetails? details,
  }) async {
    final body = <String, dynamic>{
      'name': ?name,
      'description': ?description,
      'longitude': ?position?.longitude,
      'latitude': ?position?.latitude,
      'details': ?details?.toJson(),
    };
    final r = await _client.put('/api/activities/fishing/$id', data: body);
    if (r.statusCode != 204) {
      throw Exception('Failed to update fishing activity $id: ${r.statusCode} ${r.data}');
    }
  }

  Future<void> delete(String id) async {
    final r = await _client.delete('/api/activities/fishing/$id');
    if (r.statusCode != 204) {
      throw Exception('Failed to delete fishing activity $id: ${r.statusCode}');
    }
  }
}
