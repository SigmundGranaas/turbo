import 'dart:convert';
import 'package:http/http.dart' as http;
import 'package:flutter/foundation.dart';
import 'package:latlong2/latlong.dart';
import '../model/marker.dart';

class ApiLocationService {
  final String baseUrl;
  final Future<String?> Function() tokenProvider;

  ApiLocationService({required this.baseUrl, required this.tokenProvider});

  Future<Marker?> createLocation(Marker marker) async {
    try {
      if (kDebugMode) {
        print("Creating location via API: ${marker.title}");
      }
      var token = await tokenProvider();

      final response = await http.post(
        Uri.parse('$baseUrl/api/geo/locations'),
        headers: {
          'Content-Type': 'application/json',
          'Authorization': 'Bearer $token',
        },
        body: jsonEncode({
          'location': {
            'longitude': marker.position.longitude,
            'latitude': marker.position.latitude,
          },
          'display': {
            'name': marker.title,
            'description': marker.description,
            'icon': marker.icon,
          },
        }),
      );

      if (kDebugMode) {
        print("Create location response: ${response.statusCode} - ${response.body}");
      }

      if (response.statusCode == 201) {
        final data = jsonDecode(response.body);
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
      var token = await tokenProvider();

      final response = await http.put(
        Uri.parse('$baseUrl/api/geo/locations/${marker.uuid}/position'),
        headers: {
          'Content-Type': 'application/json',
          'Authorization': 'Bearer $token',
        },
        body: jsonEncode({
          'Location': {
            'Longitude': marker.position.longitude,
            'Latitude': marker.position.latitude,
          },
          'Name': marker.title,
          'Description': marker.description,
          'Icon': marker.icon
        }),
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
      var token = await tokenProvider();

      final response = await http.delete(
        Uri.parse('$baseUrl/api/geo/locations/$id'),
        headers: {
          'Authorization': 'Bearer $token',
        },
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
      var token = await tokenProvider();

      final response = await http.get(
        Uri.parse('$baseUrl/api/geo/locations?minLon=$minLon&minLat=$minLat&maxLon=$maxLon&maxLat=$maxLat'),
        headers: {
          'Authorization': 'Bearer $token',
        },
      );

      if (kDebugMode) {
        print("Get locations response: ${response.statusCode}");
      }

      if (response.statusCode == 200) {
        final List<dynamic> data = jsonDecode(response.body);
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
          }

          if (kDebugMode) {
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

      var token = await tokenProvider();


      final response = await http.get(
        Uri.parse('$baseUrl/api/geo/locations/$id'),
        headers: {
          'Authorization': 'Bearer $token',
        },
      );

      if (kDebugMode) {
        print("Get location response: ${response.statusCode}");
      }

      if (response.statusCode == 200) {
        final data = jsonDecode(response.body);

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