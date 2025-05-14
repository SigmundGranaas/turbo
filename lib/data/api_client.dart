import 'package:dio/dio.dart';
import 'package:flutter/foundation.dart';
import 'package:shared_preferences/shared_preferences.dart';
import 'env_config.dart';

class ApiClient {
  final String baseUrl;
  late Dio _dio;
  bool _isRefreshing = false;
  final List<RequestOptions> _pendingRequests = [];

  ApiClient({String? baseUrl}) : baseUrl = baseUrl ?? EnvironmentConfig.apiBaseUrl {
    _dio = _createDio();
  }

  Dio _createDio() {
    final dio = Dio();
    dio.options.baseUrl = baseUrl;
    dio.options.connectTimeout = const Duration(seconds: 15);
    dio.options.receiveTimeout = const Duration(seconds: 15);
    dio.options.validateStatus = (status) => status != null && status < 500;
    dio.options.receiveDataWhenStatusError = true;
    dio.options.followRedirects = false;

    // For web, ensure cookies are sent with requests
    if (kIsWeb) {
      dio.options.extra['withCredentials'] = true;
    } else {
      // For mobile, add token to requests
      dio.interceptors.add(
        InterceptorsWrapper(
          onRequest: (options, handler) async {
            final prefs = await SharedPreferences.getInstance();
            final accessToken = prefs.getString('accessToken');
            if (accessToken != null) {
              options.headers['Authorization'] = 'Bearer $accessToken';
            }
            return handler.next(options);
          },
          onError: (DioException error, handler) async {
            // Handle 401 errors (token expired)
            if (error.response?.statusCode == 401) {
              // Skip token refresh for auth endpoints
              if (error.requestOptions.path.contains('/api/auth/refresh') ||
                  error.requestOptions.path.contains('/api/auth/login') ||
                  error.requestOptions.path.contains('/api/auth/register')) {
                return handler.next(error);
              }

              // Don't try to refresh if we're already in the process
              if (_isRefreshing) {
                // Queue the request for later retry
                _pendingRequests.add(error.requestOptions);
                return handler.next(error);
              }

              // Try to refresh the token
              try {
                final refreshed = await refreshToken();
                if (refreshed) {
                  // Retry the original request with new token
                  final response = await _retryRequest(error.requestOptions);

                  // Process any pending requests
                  _processPendingRequests();

                  return handler.resolve(response);
                }
              } catch (e) {
                if (kDebugMode) {
                  print('Token refresh error: $e');
                }
                // Token refresh failed, let the error propagate
              }
            }
            return handler.next(error);
          },
        ),
      );
    }
    return dio;
  }

  Future<Response> _retryRequest(RequestOptions requestOptions) async {
    final prefs = await SharedPreferences.getInstance();
    final newToken = prefs.getString('accessToken');

    final options = Options(
      method: requestOptions.method,
      headers: {...requestOptions.headers},
    );

    if (!kIsWeb && newToken != null) {
      options.headers?['Authorization'] = 'Bearer $newToken';
    }

    return _dio.request(
      requestOptions.path,
      data: requestOptions.data,
      queryParameters: requestOptions.queryParameters,
      options: options,
    );
  }

  void _processPendingRequests() async {
    final requests = List<RequestOptions>.from(_pendingRequests);
    _pendingRequests.clear();

    for (var request in requests) {
      try {
        await _retryRequest(request);
      } catch (e) {
        if (kDebugMode) {
          print('Error retrying request: $e');
        }
      }
    }
  }

  Future<bool> refreshToken() async {
    if (_isRefreshing) return false;
    _isRefreshing = true;

    try {
      // For web, the cookie is automatically sent
      // For mobile, we need to send the refresh token
      final refreshData = <String, dynamic>{};

      if (!kIsWeb) {
        final prefs = await SharedPreferences.getInstance();
        final refreshToken = prefs.getString('refreshToken');
        if (refreshToken == null) {
          _isRefreshing = false;
          return false;
        }
        refreshData['refreshToken'] = refreshToken;
      }

      // Create a separate Dio instance for the refresh call to avoid interceptor loops
      final refreshDio = Dio();
      refreshDio.options.baseUrl = baseUrl;

      if (kIsWeb) {
        refreshDio.options.extra['withCredentials'] = true;
      }

      final response = await refreshDio.post(
        '/api/auth/refresh',
        data: refreshData.isNotEmpty ? refreshData : null,
        options: Options(
          headers: {'Content-Type': 'application/json'},
          validateStatus: (status) => status != null && status < 500,
        ),
      );

      if (kDebugMode) {
        print('Refresh token response: ${response.statusCode}');
      }

      // For mobile, we need to store the new tokens
      if (!kIsWeb && response.statusCode == 200 && response.data['success'] == true) {
        final prefs = await SharedPreferences.getInstance();
        if (response.data['accessToken'] != null) {
          await prefs.setString('accessToken', response.data['accessToken']);
        }
        if (response.data['refreshToken'] != null) {
          await prefs.setString('refreshToken', response.data['refreshToken']);
        }
        _isRefreshing = false;
        return true;
      }

      // For web, we just need to check success as cookies are handled automatically
      if (kIsWeb && response.statusCode == 200 && response.data['success'] == true) {
        _isRefreshing = false;
        return true;
      }

      _isRefreshing = false;
      return false;
    } catch (e) {
      if (kDebugMode) {
        print('Error refreshing token: $e');
      }
      _isRefreshing = false;
      return false;
    }
  }

  Future<Response> get(String path, {Map<String, dynamic>? queryParameters}) async {
    return _dio.get(path, queryParameters: queryParameters);
  }

  Future<Response> post(String path, {Object? data, Options? options}) async {
    return _dio.post(
      path,
      data: data,
      options: options ?? Options(headers: {'Content-Type': 'application/json'}),
    );
  }

  Future<Response> put(String path, {Object? data, Options? options}) async {
    return _dio.put(
      path,
      data: data,
      options: options ?? Options(headers: {'Content-Type': 'application/json'}),
    );
  }

  Future<Response> delete(String path, {Object? data, Options? options}) async {
    return _dio.delete(path, data: data, options: options);
  }
}