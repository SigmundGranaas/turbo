import 'package:flutter/foundation.dart';
import 'package:shared_preferences/shared_preferences.dart';

import '../api_client.dart';

class AuthService {
  final String baseUrl;
  final ApiClient apiClient;

  AuthService({required this.baseUrl})
      : apiClient = ApiClient(baseUrl: baseUrl);

  // Expose the ApiClient as a getter
  ApiClient get client => apiClient;

  Future<bool> register(String email, String password, String confirmPassword) async {
    try {
      if (kDebugMode) {
        print("Attempting to register with email: $email");
      }

      final response = await apiClient.post(
        '/api/auth/register',
        data: {
          'email': email,
          'password': password,
          'confirmPassword': confirmPassword
        },
      );

      if (kDebugMode) {
        print("Register response: ${response.statusCode}");
      }

      final data = response.data;
      if (data['success'] == true) {
        // In web, we rely on cookies, while on mobile we store tokens
        if (!kIsWeb && data['accessToken'] != null && data['refreshToken'] != null) {
          final prefs = await SharedPreferences.getInstance();
          await prefs.setString('accessToken', data['accessToken']);
          await prefs.setString('refreshToken', data['refreshToken']);
        }

        // Store login state
        final prefs = await SharedPreferences.getInstance();
        await prefs.setBool('isLoggedIn', true);
        await prefs.setString('userEmail', email);

        return true;
      }

      throw Exception(data['error'] ?? 'Registration failed');
    } catch (e) {
      if (kDebugMode) {
        print("Registration error: $e");
      }
      rethrow;
    }
  }

  Future<bool> login(String email, String password) async {
    try {
      if (kDebugMode) {
        print("Attempting to login with email: $email");
      }

      final response = await apiClient.post(
        '/api/auth/login',
        data: {
          'email': email,
          'password': password
        },
      );

      if (kDebugMode) {
        print("Login response: ${response.statusCode}");
        if (kIsWeb) {
          print("Cookies: ${response.headers['set-cookie']}");
        }
      }

      final data = response.data;
      if (data['success'] == true) {
        // For mobile, store tokens
        if (!kIsWeb && data['accessToken'] != null && data['refreshToken'] != null) {
          final prefs = await SharedPreferences.getInstance();
          await prefs.setString('accessToken', data['accessToken']);
          await prefs.setString('refreshToken', data['refreshToken']);
        }

        // Store login state
        final prefs = await SharedPreferences.getInstance();
        await prefs.setBool('isLoggedIn', true);
        await prefs.setString('userEmail', email);

        return true;
      }

      throw Exception(data['error'] ?? 'Login failed');
    } catch (e) {
      if (kDebugMode) {
        print("Login error: $e");
      }
      rethrow;
    }
  }

  Future<bool> loginWithGoogle(String idToken) async {
    try {
      if (kDebugMode) {
        print("Attempting to login with Google ID token");
      }

      final response = await apiClient.post(
        '/api/auth/google/login',
        data: {
          'idToken': idToken
        },
      );

      if (kDebugMode) {
        print("Google login response: ${response.statusCode}");
      }

      final data = response.data;
      if (data['success'] == true) {
        // For mobile, store tokens
        if (!kIsWeb && data['accessToken'] != null && data['refreshToken'] != null) {
          final prefs = await SharedPreferences.getInstance();
          await prefs.setString('accessToken', data['accessToken']);
          await prefs.setString('refreshToken', data['refreshToken']);
        }

        // Store login state
        final prefs = await SharedPreferences.getInstance();
        await prefs.setBool('isLoggedIn', true);
        await prefs.setBool('isGoogleUser', true);

        // Store email if provided in response, otherwise mark as Google user
        final email = data['email'] ?? 'google_user';
        await prefs.setString('userEmail', email);

        return true;
      }

      throw Exception(data['error'] ?? 'Google login failed');
    } catch (e) {
      if (kDebugMode) {
        print("Google login error: $e");
      }
      rethrow;
    }
  }

  Future<String> getGoogleAuthUrl() async {
    try {
      if (kDebugMode) {
        print("Getting Google auth URL");
      }

      final response = await apiClient.get('/api/auth/google/url');

      if (kDebugMode) {
        print("Google auth URL response: ${response.statusCode}");
      }

      if (response.statusCode == 200) {
        // Handle different response formats
        if (response.data is String) {
          return response.data.replaceAll('"', '');
        } else if (response.data is Map) {
          return response.data['url'] ?? '';
        }
        return response.data.toString();
      }

      throw Exception('Failed to get Google auth URL');
    } catch (e) {
      if (kDebugMode) {
        print("Error getting Google auth URL: $e");
      }
      rethrow;
    }
  }

  Future<bool> checkOAuthSuccess() async {
    try {
      if (kDebugMode) {
        print("Checking OAuth success status");
      }

      final response = await apiClient.get(
        '/api/auth/status',
      );

      if (response.statusCode == 200) {
        if (kDebugMode) {
          print("Auth session confirmed");
        }

        // Store successful login state
        final prefs = await SharedPreferences.getInstance();
        await prefs.setBool('isLoggedIn', true);

        return true;
      }

      return false;
    } catch (e) {
      if (kDebugMode) {
        print("Error checking OAuth success: $e");
      }
      return false;
    }
  }

  Future<void> logout() async {
    try {
      if (kDebugMode) {
        print("Attempting to logout");
      }

      await apiClient.post('/api/auth/logout');

      // Clear all auth-related data from SharedPreferences
      final prefs = await SharedPreferences.getInstance();
      await prefs.setBool('isLoggedIn', false);
      await prefs.remove('userEmail');
      await prefs.remove('accessToken');
      await prefs.remove('refreshToken');
      await prefs.remove('isGoogleUser');

      if (kDebugMode) {
        print("Logout successful - cleared local session");
      }
    } catch (e) {
      if (kDebugMode) {
        print("Logout error: $e");
      }

      // Even if the API call fails, we should clear local session data
      final prefs = await SharedPreferences.getInstance();
      await prefs.setBool('isLoggedIn', false);
      await prefs.remove('userEmail');
      await prefs.remove('accessToken');
      await prefs.remove('refreshToken');
      await prefs.remove('isGoogleUser');

      rethrow;
    }
  }

  /// Refreshes the authentication token
  /// Returns true if successful, false otherwise
  Future<bool> refreshToken() async {
    try {
      if (kDebugMode) {
        print("Attempting to refresh token");
      }

      // Use the ApiClient's refreshToken method
      return await apiClient.refreshToken();
    } catch (e) {
      if (kDebugMode) {
        print("Token refresh error: $e");
      }
      return false;
    }
  }

  Future<bool> isLoggedIn() async {
    try {
      // For web, verify auth with server
      if (kIsWeb) {
        final response = await apiClient.get('/api/auth/validate');

        if (kDebugMode) {
          print("Validate response: ${response.statusCode}");
        }

        if (response.statusCode == 200) {
          // We are logged in
          final prefs = await SharedPreferences.getInstance();
          await prefs.setBool('isLoggedIn', true);

          // Try to extract user info
          try {
            final data = response.data;
            if (kDebugMode) {
              print("Response: $data");
            }
            if (data['email'] != null) {
              await prefs.setString('userEmail', data['email']);
            }
            if (data['authType'] != null) {
              await prefs.setBool('isGoogleUser', data['authType'] == 'Google');
            }
          } catch (e) {
            // Log parsing errors
            if (kDebugMode) {
              print("Error parsing validate response: $e");
            }
          }

          return true;
        }

        // Not authenticated
        return false;
      } else {
        // For mobile/desktop, check stored flag
        final prefs = await SharedPreferences.getInstance();
        final isLoggedIn = prefs.getBool('isLoggedIn') ?? false;

        if (kDebugMode) {
          print("isLoggedIn check: $isLoggedIn");
        }

        return isLoggedIn;
      }
    } catch (e) {
      if (kDebugMode) {
        print("Error checking login status: $e");
      }
      return false;
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
    if (kIsWeb) return null; // Web uses cookies

    final prefs = await SharedPreferences.getInstance();
    return prefs.getString('accessToken');
  }
}