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
      '/api/auth/register',
      data: {
        'email': email,
        'password': password,
        'confirmPassword': confirmPassword
      },
    );
    if (response.statusCode == 200 || response.statusCode == 201) {
      final data = response.data;
      if (data['success'] == true) {
        if (!kIsWeb && data['accessToken'] != null && data['refreshToken'] != null) {
          final prefs = await SharedPreferences.getInstance();
          await prefs.setString('accessToken', data['accessToken']);
          await prefs.setString('refreshToken', data['refreshToken']);
        }
        final prefs = await SharedPreferences.getInstance();
        await prefs.setBool('isLoggedIn', true);
        await prefs.setString('userEmail', email);
        return true;
      }
    }
    throw Exception(response.data['error'] ?? 'Registration failed');
  }

  Future<bool> login(String email, String password) async {
    final response = await apiClient.post(
      '/api/auth/login',
      data: {'email': email, 'password': password},
    );

    if (response.statusCode == 200) {
      final data = response.data;
      if (data['success'] == true) {
        if (!kIsWeb && data['accessToken'] != null && data['refreshToken'] != null) {
          final prefs = await SharedPreferences.getInstance();
          await prefs.setString('accessToken', data['accessToken']);
          await prefs.setString('refreshToken', data['refreshToken']);
        }
        final prefs = await SharedPreferences.getInstance();
        await prefs.setBool('isLoggedIn', true);
        await prefs.setString('userEmail', email);
        return true;
      }
    }
    throw Exception(response.data['error'] ?? 'Login failed');
  }

  Future<bool> loginWithGoogle(String idToken) async {
    final response = await apiClient.post(
      '/api/auth/google/login',
      data: {'idToken': idToken},
    );
    if (response.statusCode == 200) {
      final data = response.data;
      if (data['success'] == true) {
        if (!kIsWeb && data['accessToken'] != null && data['refreshToken'] != null) {
          final prefs = await SharedPreferences.getInstance();
          await prefs.setString('accessToken', data['accessToken']);
          await prefs.setString('refreshToken', data['refreshToken']);
        }
        final prefs = await SharedPreferences.getInstance();
        await prefs.setBool('isLoggedIn', true);
        await prefs.setBool('isGoogleUser', true);
        final email = data['email'] ?? 'google_user';
        await prefs.setString('userEmail', email);
        return true;
      }
    }
    throw Exception(response.data['error'] ?? 'Google login failed');
  }

  Future<String> getGoogleAuthUrl() async {
    final response = await apiClient.get('/api/auth/google/url');
    if (response.statusCode == 200) {
      if (response.data is String) return response.data.replaceAll('"', '');
      if (response.data is Map) return response.data['url'] ?? '';
      return response.data.toString();
    }
    throw Exception('Failed to get Google auth URL');
  }

  Future<void> logout() async {
    try {
      await apiClient.post('/api/auth/logout');
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
    return await apiClient.refreshToken();
  }

  Future<bool> isLoggedIn() async {
    if (kIsWeb) {
      try {
        final response = await apiClient.get('/api/auth/validate');
        if (response.statusCode == 200) {
          final prefs = await SharedPreferences.getInstance();
          await prefs.setBool('isLoggedIn', true);
          final data = response.data;
          if (data['email'] != null) await prefs.setString('userEmail', data['email']);
          if (data['authType'] != null) await prefs.setBool('isGoogleUser', data['authType'] == 'Google');
          return true;
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

  Future<String?> getAccessToken() async {
    if (kIsWeb) return null;
    final prefs = await SharedPreferences.getInstance();
    return prefs.getString('accessToken');
  }
}