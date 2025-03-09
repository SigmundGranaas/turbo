// auth_service.dart
import 'dart:convert';
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
        return true;
      }
      throw Exception(data['error'] ?? 'Login failed');
    } catch (e) {
      if (kDebugMode) {
        print("Login error: $e");
      }

      // For testing purposes, simulate successful login in debug mode
      if (kDebugMode) {
        print("DEBUG MODE: Simulating successful login");
        final prefs = await SharedPreferences.getInstance();
        await prefs.setBool('isLoggedIn', true);
        await prefs.setString('userEmail', email);
        return true;
      }

      rethrow;
    }
  }

  Future<void> logout() async {
    try {
      if (kDebugMode) {
        print("Attempting to logout");
      }

      try {
        await http.post(
          Uri.parse('$baseUrl/api/auth/logout'),
          headers: {'Content-Type': 'application/json'},
        );
      } catch (e) {
        if (kDebugMode) {
          print("API logout error (will still clear local session): $e");
        }
      }

      final prefs = await SharedPreferences.getInstance();
      await prefs.setBool('isLoggedIn', false);
      await prefs.remove('userEmail');

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

      final response = await http.post(
        Uri.parse('$baseUrl/api/auth/refresh'),
        headers: {'Content-Type': 'application/json'},
      );

      final data = jsonDecode(response.body);
      return data['success'] ?? false;
    } catch (e) {
      if (kDebugMode) {
        print("Token refresh error: $e");
      }
      return false;
    }
  }

  Future<bool> isLoggedIn() async {
    final prefs = await SharedPreferences.getInstance();
    final isLoggedIn = prefs.getBool('isLoggedIn') ?? false;

    if (kDebugMode) {
      print("isLoggedIn check: $isLoggedIn");
    }

    return isLoggedIn;
  }

  Future<String?> getUserEmail() async {
    final prefs = await SharedPreferences.getInstance();
    return prefs.getString('userEmail');
  }
}