import 'package:flutter/foundation.dart';
import 'package:shared_preferences/shared_preferences.dart';
import '../api_client.dart';
import '../env_config.dart';

class AuthService {
  final String baseUrl;
  final ApiClient apiClient;

  AuthService({String? baseUrl})
      : baseUrl = baseUrl ?? EnvironmentConfig.apiBaseUrl,
        apiClient = ApiClient(baseUrl: baseUrl ?? EnvironmentConfig.apiBaseUrl);

  ApiClient get client => apiClient;

  Future<bool> register(String email, String password, String confirmPassword) async {
    final response = await apiClient.post(
      '/api/auth/Auth/register',
      data: {
        'email': email,
        'password': password,
        'confirmPassword': confirmPassword
      },
    );
    if (response.statusCode == 200) {
      final data = response.data;
      if (data['accessToken'] != null) {
        if (!kIsWeb) {
          await storeTokens(data['accessToken'], data['refreshToken']);
        }
        await setLoginState(email: data['email'], isGoogleUser: false);
        return true;
      }
    }
    throw Exception(response.data['message'] ?? 'Registration failed');
  }

  Future<bool> login(String email, String password) async {
    final response = await apiClient.post(
      '/api/auth/Auth/login',
      data: {'email': email, 'password': password},
    );

    if (response.statusCode == 200) {
      final data = response.data;
      if (data['accessToken'] != null) {
        if (!kIsWeb) {
          await storeTokens(data['accessToken'], data['refreshToken']);
        }
        await setLoginState(email: data['email'], isGoogleUser: false);
        return true;
      }
    }
    throw Exception(response.data['message'] ?? 'Login failed');
  }

  Future<String> getGoogleAuthUrl() async {
    final response = await apiClient.get('/api/auth/OAuth/google/url');
    if (response.statusCode == 200) {
      if (response.data is Map && response.data['authorizationUrl'] != null) {
        return response.data['authorizationUrl'];
      }
      return response.data.toString();
    }
    throw Exception('Failed to get Google auth URL');
  }

  Future<void> logout() async {
    try {
      if (!kIsWeb) {
        final prefs = await SharedPreferences.getInstance();
        final refreshToken = prefs.getString('refreshToken');
        if (refreshToken != null) {
          await apiClient.post('/api/auth/Token/revoke', data: {'refreshToken': refreshToken});
        }
      } else {
        await apiClient.post('/api/auth/Token/revoke');
      }
    } finally {
      final prefs = await SharedPreferences.getInstance();
      await prefs.setBool('isLoggedIn', false);
      await prefs.remove('userEmail');
      await prefs.remove('accessToken');
      await prefs.remove('refreshToken');
      await prefs.remove('isGoogleUser');
    }
  }

  Future<bool> refreshToken() async {
    // This now points to the correctly proxied path
    return await apiClient.refreshToken();
  }

  Future<bool> isLoggedIn() async {
    if (kIsWeb) {
      try {
        final response = await apiClient.get('/api/auth/Session/me');
        if (response.statusCode == 200) {
          final data = response.data;
          if (data['isActive'] == true) {
            final prefs = await SharedPreferences.getInstance();
            await prefs.setBool('isLoggedIn', true);
            if (data['email'] != null) {
              await prefs.setString('userEmail', data['email']);
            }
            return true;
          }
        }
        return false;
      } catch (e) {
        return false;
      }
    } else {
      final prefs = await SharedPreferences.getInstance();
      return prefs.getBool('isLoggedIn') ?? false;
    }
  }

  Future<String?> getUserEmail() async {
    final prefs = await SharedPreferences.getInstance();
    return prefs.getString('userEmail');
  }

  Future<bool> isGoogleUser() async {
    final prefs = await SharedPreferences.getInstance();
    return prefs.getBool('isGoogleUser') ?? false;
  }

  Future<void> setIsGoogleUser(bool isGoogle) async {
    final prefs = await SharedPreferences.getInstance();
    await prefs.setBool('isGoogleUser', isGoogle);
  }

  Future<String?> getAccessToken() async {
    if (kIsWeb) return null;
    final prefs = await SharedPreferences.getInstance();
    return prefs.getString('accessToken');
  }

  Future<void> storeTokens(String accessToken, String refreshToken) async {
    if (kIsWeb) return;
    final prefs = await SharedPreferences.getInstance();
    await prefs.setString('accessToken', accessToken);
    await prefs.setString('refreshToken', refreshToken);
  }

  Future<void> setLoginState({required String email, required bool isGoogleUser}) async {
    final prefs = await SharedPreferences.getInstance();
    await prefs.setBool('isLoggedIn', true);
    await prefs.setString('userEmail', email);
    await prefs.setBool('isGoogleUser', isGoogleUser);
  }
}