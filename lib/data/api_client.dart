import 'package:dio/dio.dart';
import 'package:flutter/foundation.dart';
import 'package:shared_preferences/shared_preferences.dart';
import 'env_config.dart';

class ApiClient {
  final String baseUrl;
  late Dio _dio;
  bool _isRefreshing = false;

  // Callback for when authentication fails irrecoverably (e.g., refresh token fails).
  Function? _onAuthFailure;

  ApiClient({String? baseUrl}) : baseUrl = baseUrl ?? EnvironmentConfig.apiBaseUrl {
    _dio = _createDio();
  }

  /// Sets a handler to be called on final authentication failure.
  void setAuthFailureHandler(Function handler) {
    _onAuthFailure = handler;
  }

  Dio _createDio() {
    final dio = Dio();
    dio.options.baseUrl = baseUrl;
    dio.options.connectTimeout = const Duration(seconds: 5);
    dio.options.receiveTimeout = const Duration(seconds: 8);
    dio.options.receiveDataWhenStatusError = true;
    dio.options.followRedirects = false;
    dio.options.headers['User-Agent'] = 'turbo_map_app/1.0.18';

    // For web, ensure cookies are sent with requests
    if (kIsWeb) {
      dio.options.extra['withCredentials'] = true;
    }

    dio.interceptors.add(
      // Use QueuedInterceptorsWrapper to handle concurrent requests during token refresh.
      QueuedInterceptorsWrapper(
        onRequest: (options, handler) async {
          if (!kIsWeb) {
            final prefs = await SharedPreferences.getInstance();
            final accessToken = prefs.getString('accessToken');
            if (accessToken != null) {
              options.headers['Authorization'] = 'Bearer $accessToken';
            }
          }
          return handler.next(options);
        },
        onError: (DioException error, handler) async {
          final path = error.requestOptions.path;

          if (kDebugMode) {
            print("Interceptet error: ${error.message} ${error.response?.statusCode}");

          }
          // Check for 401 Unauthorized, but ignore for auth endpoints to prevent loops.
          if (error.response?.statusCode == 401 && !path.contains('/api/auth/Token/refresh')) {
            // On mobile, only try to refresh if a refresh token exists.
            if (!kIsWeb) {
              final prefs = await SharedPreferences.getInstance();
              if (prefs.getString('refreshToken') == null) {
                return handler.next(error);
              }
            }

            if (!_isRefreshing) {
              _isRefreshing = true;

              try {
                final refreshed = await _performTokenRefresh();
                if (refreshed) {
                  // If refresh is successful, retry the original request.
                  // The QueuedInterceptorsWrapper will process pending requests.
                  final response = await _retryRequest(error.requestOptions);
                  return handler.resolve(response);
                } else {
                  // Refresh failed, trigger logout and reject the request.
                  _onAuthFailure?.call();
                  return handler.reject(error);
                }
              } catch (e) {
                _onAuthFailure?.call();
                return handler.reject(error);
              } finally {
                // Always reset the flag.
                _isRefreshing = false;
              }
            } else {
              // A refresh is already in progress. The request is queued.
              // We retry it, and it will wait for the first refresh to complete.
              try {
                final response = await _retryRequest(error.requestOptions);
                return handler.resolve(response);
              } on DioException catch (e) {
                return handler.reject(e);
              }
            }
          }
          return handler.next(error);
        },
      ),
    );
    return dio;
  }

  Future<Response> _retryRequest(RequestOptions requestOptions) async {
    final options = Options(
      method: requestOptions.method,
      headers: requestOptions.headers,
    );

    // Get the new token for the retry
    if (!kIsWeb) {
      final prefs = await SharedPreferences.getInstance();
      final newAccessToken = prefs.getString('accessToken');
      if (newAccessToken != null) {
        options.headers?['Authorization'] = 'Bearer $newAccessToken';
      }
    }

    // Use a new Dio instance for retrying to avoid interceptor loops if something goes wrong
    // Or, more simply, just use the original dio instance but be careful.
    // Since we're just re-requesting with new headers, using _dio is fine.
    return _dio.request<dynamic>(
      requestOptions.path,
      data: requestOptions.data,
      queryParameters: requestOptions.queryParameters,
      options: options,
    );
  }

  // In lib/data/api_client.dart

  Future<bool> _performTokenRefresh() async {
    try {
      Object? requestData;

      if (kIsWeb) {
        // For web, we don't send a body with a refresh token.
        // We rely on the HttpOnly cookie being sent automatically by the browser.
        // However, we send an empty map `{}` as the body, as many backends
        // require a valid JSON body for POST requests, even if empty.
        requestData = {};
      } else {
        // For mobile, we get the refresh token from local storage.
        final prefs = await SharedPreferences.getInstance();
        final refreshToken = prefs.getString('refreshToken');
        if (refreshToken == null) {
          if (kDebugMode) print('No refresh token found on mobile, refresh failed.');
          return false;
        }
        requestData = {'refreshToken': refreshToken};
      }

      if (kDebugMode) {
        print('Attempting token refresh. Platform: ${kIsWeb ? 'Web' : 'Mobile'}.');
      }

      // Use the main dio instance, as the interceptor handles the recursion check.
      final response = await _dio.post(
        '/api/auth/Token/refresh',
        data: requestData,
      );

      if (kDebugMode) {
        print('Refresh token response: ${response.statusCode}');
      }

      if (response.statusCode == 200) {
        if (!kIsWeb) {
          // For mobile, store the new tokens.
          final data = response.data;
          if (data['accessToken'] != null && data['refreshToken'] != null) {
            final prefs = await SharedPreferences.getInstance();
            await prefs.setString('accessToken', data['accessToken']);
            await prefs.setString('refreshToken', data['refreshToken']);
            if (kDebugMode) print('Mobile tokens refreshed and stored.');
            return true;
          }
        } else {
          // For web, a 200 OK is enough as cookies are handled by the browser.
          if (kDebugMode) print('Web token refresh successful (cookies updated by server).');
          return true;
        }
      }
      // If we reach here, the refresh failed.
      if (kDebugMode) {
        print('Token refresh failed with status: ${response.statusCode}. Body: ${response.data}');
      }
      return false;
    } catch (e) {
      if (kDebugMode) {
        print('Error exception during token refresh: $e');
      }
      return false;
    }
  }

  /// Manually triggers a token refresh. Used for mobile app resume.
  Future<bool> refreshToken() async {
    if (_isRefreshing) return false;
    _isRefreshing = true;
    try {
      return await _performTokenRefresh();
    } finally {
      _isRefreshing = false;
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