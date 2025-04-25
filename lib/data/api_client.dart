import 'package:dio/dio.dart';
import 'package:flutter/foundation.dart';
import 'package:shared_preferences/shared_preferences.dart';

class ApiClient {
  final String baseUrl;
  late Dio _dio;

  ApiClient({required this.baseUrl}) {
    _dio = _createDio();
  }

  Dio _createDio() {
    final dio = Dio();
    dio.options.baseUrl = baseUrl;
    dio.options.validateStatus = (status) => true;
    dio.options.receiveDataWhenStatusError = true;
    dio.options.followRedirects = false;

    // For web, include credentials in requests to handle cookies
    if (kIsWeb) {
      dio.options.extra['withCredentials'] = true;
    }

    // Add interceptors for handling auth tokens on non-web platforms
    if (!kIsWeb) {
      dio.interceptors.add(
        InterceptorsWrapper(
          onRequest: (options, handler) async {
            // Add auth token for non-web platforms
            final prefs = await SharedPreferences.getInstance();
            final accessToken = prefs.getString('accessToken');
            if (accessToken != null) {
              options.headers['Authorization'] = 'Bearer $accessToken';
            }
            return handler.next(options);
          },
          onError: (DioException error, handler) async {
            // Handle 401 errors by attempting to refresh the token
            if (error.response?.statusCode == 401) {
              try {
                final refreshed = await refreshToken();
                if (refreshed) {
                  // Retry the original request with new token
                  final prefs = await SharedPreferences.getInstance();
                  final newAccessToken = prefs.getString('accessToken');

                  final options = error.requestOptions;
                  options.headers['Authorization'] = 'Bearer $newAccessToken';

                  // Create a new request with the updated token
                  final response = await dio.fetch(options);
                  return handler.resolve(response);
                }
              } catch (e) {
                if (kDebugMode) {
                  print('Failed to refresh token: $e');
                }
              }
            }
            return handler.next(error);
          },
        ),
      );
    }

    return dio;
  }

  /// Refreshes the access token using the refresh token
  /// Returns true if successful, false otherwise
  Future<bool> refreshToken() async {
    try {
      // For web platforms using cookies, no token refresh is needed
      if (kIsWeb) {
        return true;
      }

      final prefs = await SharedPreferences.getInstance();
      final refreshToken = prefs.getString('refreshToken');

      if (refreshToken == null) {
        return false;
      }

      // Create a clean Dio instance without the auth interceptor to avoid loops
      final refreshDio = Dio();
      refreshDio.options.baseUrl = baseUrl;

      final response = await refreshDio.post(
        '/api/auth/refresh',
        data: {'refreshToken': refreshToken},
        options: Options(
          headers: {'Content-Type': 'application/json'},
        ),
      );

      if (response.statusCode == 200 && response.data['success'] == true) {
        // Update tokens in storage
        if (response.data['accessToken'] != null && response.data['refreshToken'] != null) {
          await prefs.setString('accessToken', response.data['accessToken']);
          await prefs.setString('refreshToken', response.data['refreshToken']);
          return true;
        }
      }
      return false;
    } catch (e) {
      if (kDebugMode) {
        print('Error refreshing token: $e');
      }
      return false;
    }
  }

  // GET request with authorization
  Future<Response> get(String path, {Map<String, dynamic>? queryParameters}) async {
    try {
      return await _dio.get(
        path,
        queryParameters: queryParameters,
      );
    } catch (e) {
      if (kDebugMode) {
        print('GET request error: $e');
      }
      rethrow;
    }
  }

  // POST request with authorization
  Future<Response> post(String path, {Object? data, Options? options}) async {
    try {
      return await _dio.post(
        path,
        data: data,
        options: options ?? Options(headers: {'Content-Type': 'application/json'}),
      );
    } catch (e) {
      if (kDebugMode) {
        print('POST request error: $e');
      }
      rethrow;
    }
  }

  // PUT request with authorization
  Future<Response> put(String path, {Object? data, Options? options}) async {
    try {
      return await _dio.put(
        path,
        data: data,
        options: options ?? Options(headers: {'Content-Type': 'application/json'}),
      );
    } catch (e) {
      if (kDebugMode) {
        print('PUT request error: $e');
      }
      rethrow;
    }
  }

  // DELETE request with authorization
  Future<Response> delete(String path, {Object? data, Options? options}) async {
    try {
      return await _dio.delete(
        path,
        data: data,
        options: options,
      );
    } catch (e) {
      if (kDebugMode) {
        print('DELETE request error: $e');
      }
      rethrow;
    }
  }

  // Helper method to get auth token - useful for services that need it
  Future<String?> getAuthToken() async {
    if (kIsWeb) {
      // For web we're using cookies, so no token is needed
      return null;
    } else {
      final prefs = await SharedPreferences.getInstance();
      return prefs.getString('accessToken');
    }
  }
}