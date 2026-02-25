
import 'package:latlong2/latlong.dart';
import 'package:turbo/core/api/api_client.dart';
import '../models/marker.dart';

class ApiLocationService {
  final ApiClient _apiClient;

  ApiLocationService(this._apiClient);

  Future<Marker?> createLocation(Marker marker) async {
    final requestData = {
      'geometry': {
        'longitude': marker.position.longitude,
        'latitude': marker.position.latitude,
      },
      'display': {
        'name': marker.title,
        'description': marker.description,
        'icon': marker.icon,
      },
    };
    final response = await _apiClient.post('/api/geo/locations', data: requestData);

    if (response.statusCode == 201) {
      return Marker.fromApiResponse(response.data as Map<String, dynamic>);
    } else if (response.data != null && response.data['detail'] != null) {
      throw Exception('Failed to create location: ${response.data['detail']}');
    }
    throw Exception('Failed to create location: Status ${response.statusCode}');
  }

  Future<Marker?> updateLocation(Marker marker) async {
    final requestData = {
      'geometry': {
        'longitude': marker.position.longitude,
        'latitude': marker.position.latitude,
      },
      'display': {
        'name': marker.title,
        'description': marker.description,
        'icon': marker.icon,
      },
    };
    final response = await _apiClient.put('/api/geo/locations/${marker.uuid}', data: requestData);

    if (response.statusCode == 200) {
      return Marker.fromApiResponse(response.data as Map<String, dynamic>);
    } else if (response.statusCode == 404) {
      return null;
    } else if (response.data != null && response.data['detail'] != null) {
      throw Exception('Failed to update location: ${response.data['detail']}');
    }
    throw Exception('Failed to update location: Status ${response.statusCode}');
  }

  Future<bool> deleteLocation(String uuid) async {
    final response = await _apiClient.delete('/api/geo/locations/$uuid');
    if (response.statusCode == 204) {
      return true;
    } else if (response.statusCode == 404) {
      return false;
    } else if (response.data != null && response.data['detail'] != null) {
      throw Exception('Failed to delete location: ${response.data['detail']}');
    }
    throw Exception('Failed to delete location: Status ${response.statusCode}');
  }

  Future<List<Marker>> getLocationsInExtent(LatLng southwest, LatLng northeast) async {
    final queryParams = {
      'minLon': southwest.longitude.toString(),
      'minLat': southwest.latitude.toString(),
      'maxLon': northeast.longitude.toString(),
      'maxLat': northeast.latitude.toString(),
    };
    final response = await _apiClient.get('/api/geo/locations', queryParameters: queryParams);

    if (response.statusCode == 200) {
      final responseData = response.data as Map<String, dynamic>;
      final items = responseData['items'] as List<dynamic>;
      return items.map((item) => Marker.fromApiResponse(item as Map<String, dynamic>)).toList();
    } else if (response.data != null && response.data['detail'] != null) {
      throw Exception('Failed to get locations in extent: ${response.data['detail']}');
    }
    throw Exception('Failed to get locations in extent: Status ${response.statusCode}');
  }


  Future<List<Marker>> getAllUserLocations() async {
    // A common way to request all is to provide a bounding box covering the whole world
    // Or, if your API has a specific endpoint for "all user locations", use that.
    // For this example, we use a very large bounding box.
    return getLocationsInExtent(const LatLng(-90, -180), const LatLng(90, 180));
  }

  Future<Marker?> getLocationById(String uuid) async {
    final response = await _apiClient.get('/api/geo/locations/$uuid');
    if (response.statusCode == 200) {
      return Marker.fromApiResponse(response.data as Map<String, dynamic>);
    } else if (response.statusCode == 404) {
      return null;
    } else if (response.data != null && response.data['detail'] != null) {
      throw Exception('Failed to get location by ID: ${response.data['detail']}');
    }
    throw Exception('Failed to get location by ID: Status ${response.statusCode}');
  }
}