import 'dart:convert';
import 'package:dio/dio.dart';
import 'package:flutter/foundation.dart';
import 'package:http/http.dart' as http;
import 'package:shared_preferences/shared_preferences.dart';

class AuthService {
  final String baseUrl;

  AuthService({required this.baseUrl});

  Future<bool> register(String email, String password, String confirmPassword) async {
    try {
      if (kDebugMode) {
        print("Attempting to register with email: $email");
      }

      final response = await http.post(
        Uri.parse('$baseUrl/api/auth/register'),
        headers: {'Content-Type': 'application/json'},
        body: jsonEncode({
          'email': email,
          'password': password,
          'confirmPassword': confirmPassword
        }),
      );

      if (kDebugMode) {
        print("Register response: ${response.statusCode} - ${response.body}");
      }

      final data = jsonDecode(response.body);
      if (data['success'] == true) {
        // Store tokens
        if (data['accessToken'] != null && data['refreshToken'] != null) {
          final prefs = await SharedPreferences.getInstance();
          await prefs.setBool('isLoggedIn', true);
          await prefs.setString('userEmail', email);
          await prefs.setString('accessToken', data['accessToken']);
          await prefs.setString('refreshToken', data['refreshToken']);
        }
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

      final response = await http.post(
        Uri.parse('$baseUrl/api/auth/login'),
        headers: {'Content-Type': 'application/json'},
        body: jsonEncode({
          'email': email,
          'password': password
        }),
      );

      if (kDebugMode) {
        print("Login response: ${response.statusCode} - ${response.body}");
      }

      final data = jsonDecode(response.body);
      if (data['success'] == true) {
        final prefs = await SharedPreferences.getInstance();
        await prefs.setBool('isLoggedIn', true);
        await prefs.setString('userEmail', email);

        // Store tokens if they're provided
        if (data['accessToken'] != null && data['refreshToken'] != null) {
          await prefs.setString('accessToken', data['accessToken']);
          await prefs.setString('refreshToken', data['refreshToken']);
        }

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

      final response = await http.post(
        Uri.parse('$baseUrl/api/auth/google/login'),
        headers: {'Content-Type': 'application/json'},
        body: jsonEncode({
          'idToken': idToken
        }),
      );

      if (kDebugMode) {
        print("Google login response: ${response.statusCode} - ${response.body}");
      }

      final data = jsonDecode(response.body);
      if (data['success'] == true) {
        final prefs = await SharedPreferences.getInstance();
        await prefs.setBool('isLoggedIn', true);

        // Store tokens
        if (data['accessToken'] != null && data['refreshToken'] != null) {
          await prefs.setString('accessToken', data['accessToken']);
          await prefs.setString('refreshToken', data['refreshToken']);
        }

        // Store email if provided in response, otherwise mark as Google user
        final email = data['email'] ?? 'google_user';
        await prefs.setString('userEmail', email);
        await prefs.setBool('isGoogleUser', true);

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

      final response = await http.get(
        Uri.parse('$baseUrl/api/auth/google/url'),
        headers: {'Content-Type': 'application/json'},
      );

      if (kDebugMode) {
        print("Google auth URL response: ${response.statusCode} - ${response.body}");
      }

      if (response.statusCode == 200) {
        // Remove quotes if the URL is returned as a JSON string
        return response.body.replaceAll('"', '');
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

      // For web, check for auth session cookie first (quicker)
      if (kIsWeb) {
        // Check for the session cookie using JS (would require flutter_js)
        // Alternatively, make a lightweight API call:
        final response = await http.get(
          Uri.parse('$baseUrl/api/auth/status'),
          headers: {'X-Requested-With': 'XMLHttpRequest'},
        );

        if (response.statusCode == 200) {
          if (kDebugMode) {
            print("Auth session confirmed");
          }
          return true;
        }
      }

      // Fall back to regular auth check
      return await isLoggedIn();
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

      // Get tokens for API call
      final prefs = await SharedPreferences.getInstance();
      final accessToken = prefs.getString('accessToken') ?? "invalid-token";

      try {
        // Include access token if available
        final headers = {
          'Content-Type': 'application/json',
        };

        headers['Authorization'] = 'Bearer $accessToken';
      
        // For mobile
        await http.post(
          Uri.parse('$baseUrl/api/auth/logout'),
          headers: headers,
        );

      } catch (e) {
        if (kDebugMode) {
          print("API logout error (will still clear local session): $e");
        }
      }

      // Clear all auth-related data
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
      rethrow;
    }
  }

  Future<bool> refreshToken() async {
    try {
      if (kDebugMode) {
        print("Attempting to refresh token");
      }

      final prefs = await SharedPreferences.getInstance();
      final refreshToken = prefs.getString('refreshToken');

      if (refreshToken == null) {
        if (kDebugMode) {
          print("No refresh token found");
        }
        return false;
      }

      final response = await http.post(
        Uri.parse('$baseUrl/api/auth/refresh'),
        headers: {'Content-Type': 'application/json'},
        body: jsonEncode({'refreshToken': refreshToken}),
      );

      final data = jsonDecode(response.body);
      if (data['success'] == true && data['accessToken'] != null && data['refreshToken'] != null) {
        // Update tokens
        await prefs.setString('accessToken', data['accessToken']);
        await prefs.setString('refreshToken', data['refreshToken']);
        return true;
      }
      return false;
    } catch (e) {
      if (kDebugMode) {
        print("Token refresh error: $e");
      }
      return false;
    }
  }

  Future<bool> isLoggedIn() async {
    final dio = Dio();
    dio.options.baseUrl = baseUrl;

    dio.options.validateStatus = (status) => true;
    dio.options.receiveDataWhenStatusError = true;
    dio.options.followRedirects = false;
    dio.options.extra['withCredentials'] = true;

    try {
      // For web, check authentication with server
      if (kIsWeb) {
        final response = await dio.get(
          '/api/auth/validate',
          options: Options(
            headers: {'X-Requested-With': 'XMLHttpRequest'},
          ),
        );

        if (kDebugMode) {
          print("Validate response: ${response.statusCode}");
        }

        if (response.statusCode == 200) {
          // We are logged in via cookies
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
    final prefs = await SharedPreferences.getInstance();
    return prefs.getString('accessToken');
  }
}