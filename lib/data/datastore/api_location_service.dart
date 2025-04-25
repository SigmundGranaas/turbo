import 'package:flutter/foundation.dart';
import 'package:latlong2/latlong.dart';
import '../api_client.dart';
import '../model/marker.dart';

class ApiLocationService {
  final ApiClient _apiClient;

  ApiLocationService({required ApiClient apiClient})
      : _apiClient = apiClient;

  Future<Marker?> createLocation(Marker marker) async {
    try {
      if (kDebugMode) {
        print("Creating location via API: ${marker.title}");
      }

      final response = await _apiClient.post(
        '/api/geo/locations',
        data: {
          'location': {
            'longitude': marker.position.longitude,
            'latitude': marker.position.latitude,
          },
          'display': {
            'name': marker.title,
            'description': marker.description,
            'icon': marker.icon,
          },
        },
      );

      if (kDebugMode) {
        print("Create location response: ${response.statusCode} - ${response.data}");
      }

      if (response.statusCode == 201) {
        final data = response.data;

        if (kDebugMode) {
          print("Create location response data: ${marker.copyWith(
            uuid: data['id'],
            synced: true,
          ).toMap()}");
        }

        // Create a new marker with the server-assigned ID
        return marker.copyWith(
          uuid: data['id'],
          synced: true,
        );
      }

      throw Exception('Failed to create location: ${response.statusCode}');
    } catch (e) {
      if (kDebugMode) {
        print("Create location error: $e");
      }
      rethrow;
    }
  }

  Future<bool> updateLocationPosition(Marker marker) async {
    try {
      if (kDebugMode) {
        print("Updating location position via API: ${marker.uuid}");
      }

      final response = await _apiClient.put(
        '/api/geo/locations/${marker.uuid}/position',
        data: {
          'Location': {
            'Longitude': marker.position.longitude,
            'Latitude': marker.position.latitude,
          },
          'Name': marker.title,
          'Description': marker.description,
          'Icon': marker.icon
        },
      );

      if (kDebugMode) {
        print("Update location response: ${response.statusCode}");
      }

      // Consider 200 OK as well as 204 No Content as success responses
      return response.statusCode == 204 || response.statusCode == 200;
    } catch (e) {
      if (kDebugMode) {
        print("Update location error: $e");
      }
      return false;
    }
  }

  Future<bool> deleteLocation(String id) async {
    try {
      if (kDebugMode) {
        print("Deleting location via API: $id");
      }

      final response = await _apiClient.delete(
        '/api/geo/locations/$id',
      );

      if (kDebugMode) {
        print("Delete location response: ${response.statusCode}");
      }

      return response.statusCode == 204;
    } catch (e) {
      if (kDebugMode) {
        print("Delete location error: $e");
      }
      return false;
    }
  }

  Future<List<Marker>> getLocationsInExtent(
      double minLon, double minLat, double maxLon, double maxLat) async {
    try {
      if (kDebugMode) {
        print("Getting locations in extent via API");
      }

      final response = await _apiClient.get(
        '/api/geo/locations',
        queryParameters: {
          'minLon': minLon.toString(),
          'minLat': minLat.toString(),
          'maxLon': maxLon.toString(),
          'maxLat': maxLat.toString(),
        },
      );

      if (kDebugMode) {
        print("Get locations response: ${response.statusCode}");
      }

      if (response.statusCode == 200) {
        final List<dynamic> data = response.data;

        if (kDebugMode) {
          print("api id: ${data.map((item) => item['id'])}");
        }

        return data.map((item) {
          // Safely convert coordinates to double
          double latitude = 0.0;
          double longitude = 0.0;

          // Handle latitude - could be at top level or in location object
          if (item['latitude'] != null) {
            latitude = _parseDouble(item['latitude']);
          } else if (item['location'] != null && item['location']['latitude'] != null) {
            latitude = _parseDouble(item['location']['latitude']);
          }

          // Handle longitude - could be at top level or in location object
          if (item['longitude'] != null) {
            longitude = _parseDouble(item['longitude']);
          } else if (item['location'] != null && item['location']['longitude'] != null) {
            longitude = _parseDouble(item['location']['longitude']);
          }

          // Get name from proper location based on API response structure
          String title = 'Unnamed Location';
          if (item['name'] != null) {
            title = item['name'].toString();
          } else if (item['displayData'] != null && item['displayData']['name'] != null) {
            title = item['displayData']['name'].toString();
          }

          // Get description
          String description = '';
          if (item['description'] != null) {
            description = item['description'].toString();
          } else if (item['displayData'] != null && item['displayData']['description'] != null) {
            description = item['displayData']['description'].toString();
          }

          // Get icon
          String icon = '';
          if (item['icon'] != null) {
            icon = item['icon'].toString();
          } else if (item['displayData'] != null && item['displayData']['icon'] != null) {
            icon = item['displayData']['icon'].toString();
          }

          if (kDebugMode) {
            print("Converting marker: ID=${item['id']}, lat=$latitude, lng=$longitude, title=$title");
            print("From data=$item");
          }

          return Marker(
            uuid: item['id'],
            title: title,
            description: description,
            icon: icon,
            position: LatLng(latitude, longitude),
            synced: true,
          );
        }).toList();
      }

      throw Exception('Failed to load locations: ${response.statusCode}');
    } catch (e) {
      if (kDebugMode) {
        print("Get locations error: $e");
        print("Stack trace: ${StackTrace.current}");
      }
      rethrow;
    }
  }

  Future<Marker?> getLocationById(String id) async {
    try {
      if (kDebugMode) {
        print("Getting location by ID via API: $id");
      }

      final response = await _apiClient.get(
        '/api/geo/locations/$id',
      );

      if (kDebugMode) {
        print("Get location response: ${response.statusCode}");
      }

      if (response.statusCode == 200) {
        final data = response.data;

        if (kDebugMode) {
          print("API response data: $data");
        }

        // Handle different API response structures
        String title = 'Unnamed Location';
        String description = '';
        String icon = '';
        double latitude = 0.0;
        double longitude = 0.0;

        // Extract title
        if (data['display'] != null && data['display']['name'] != null) {
          title = data['display']['name'].toString();
        } else if (data['displayData'] != null && data['displayData']['name'] != null) {
          title = data['displayData']['name'].toString();
        }

        // Extract description
        if (data['display'] != null && data['display']['description'] != null) {
          description = data['display']['description'].toString();
        } else if (data['displayData'] != null && data['displayData']['description'] != null) {
          description = data['displayData']['description'].toString();
        }

        // Extract icon
        if (data['display'] != null && data['display']['icon'] != null) {
          icon = data['display']['icon'].toString();
        } else if (data['displayData'] != null && data['displayData']['icon'] != null) {
          icon = data['displayData']['icon'].toString();
        }

        // Extract coordinates
        if (data['location'] != null) {
          latitude = _parseDouble(data['location']['latitude']);
          longitude = _parseDouble(data['location']['longitude']);
        } else {
          latitude = _parseDouble(data['latitude']);
          longitude = _parseDouble(data['longitude']);
        }

        return Marker(
          uuid: data['id'],
          title: title,
          description: description,
          icon: icon,
          position: LatLng(latitude, longitude),
          synced: true,
        );
      } else if (response.statusCode == 404) {
        return null;
      }

      throw Exception('Failed to load location: ${response.statusCode}');
    } catch (e) {
      if (kDebugMode) {
        print("Get location error: $e");
      }
      rethrow;
    }
  }

  // Helper method to safely parse a value to double
  double _parseDouble(dynamic value) {
    if (value is int) {
      return value.toDouble();
    } else if (value is double) {
      return value;
    } else if (value is String) {
      return double.tryParse(value) ?? 0.0;
    }
    return 0.0;
  }
}