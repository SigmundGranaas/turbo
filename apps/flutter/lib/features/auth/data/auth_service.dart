import 'package:flutter/foundation.dart';
import 'package:logging/logging.dart';
import 'package:shared_preferences/shared_preferences.dart';
import 'package:turbo/core/api/api_client.dart';

final _log = Logger('AuthService');

class AuthService {
  final ApiClient apiClient;

  AuthService({required this.apiClient});

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
      await prefs.remove('displayName');
      await prefs.remove('accessToken');
      await prefs.remove('refreshToken');
      await prefs.remove('isGoogleUser');
    }
  }

  Future<bool> refreshToken() async {
    // This now points to the correctly proxied path
    return await apiClient.refreshToken();
  }

  Future<bool> _performSessionCheck() async {
    final response = await apiClient.get('/api/auth/Session/me');
    if (response.statusCode == 200) {
      final data = response.data;
      if (data['isActive'] == true) {
        final prefs = await SharedPreferences.getInstance();
        await prefs.setBool('isLoggedIn', true);
        if (data['email'] != null) {
          await prefs.setString('userEmail', data['email']);
        }
        // Session/me now also returns a nullable displayName; mirror it locally.
        final displayName = data['displayName'];
        if (displayName != null) {
          await prefs.setString('displayName', displayName);
        } else {
          await prefs.remove('displayName');
        }
        return true;
      }
    }
    return false;
  }

  /// Changes the password for a password-based account.
  ///
  /// Success is a 204 (no body). On any non-2xx response we throw with the
  /// server-provided message when present.
  Future<void> changePassword(
      String currentPassword, String newPassword, String confirmNewPassword) async {
    final response = await apiClient.post(
      '/api/auth/Auth/change-password',
      data: {
        'currentPassword': currentPassword,
        'newPassword': newPassword,
        'confirmNewPassword': confirmNewPassword,
      },
    );
    final status = response.statusCode ?? 0;
    if (status >= 200 && status < 300) {
      return;
    }
    throw Exception(response.data?['message'] ?? 'Could not change password');
  }

  /// Updates the user's display name. Pass null/empty to clear it. Returns the
  /// new display name as confirmed by the server, persisting it locally.
  Future<String?> updateDisplayName(String? displayName) async {
    final response = await apiClient.put(
      '/api/auth/Profile',
      data: {'displayName': displayName ?? ''},
    );
    if (response.statusCode == 200) {
      final data = response.data;
      final newDisplayName = data['displayName'] as String?;
      final prefs = await SharedPreferences.getInstance();
      if (newDisplayName != null && newDisplayName.isNotEmpty) {
        await prefs.setString('displayName', newDisplayName);
      } else {
        await prefs.remove('displayName');
      }
      return newDisplayName;
    }
    throw Exception(response.data?['message'] ?? 'Could not update profile');
  }

  /// Registers a push device token with the backend. Not yet wired into any
  /// flow — FCM integration is handled separately.
  Future<void> registerDevice(String token, String platform) async {
    final response = await apiClient.post(
      '/api/auth/Devices',
      data: {'token': token, 'platform': platform},
    );
    final status = response.statusCode ?? 0;
    if (status >= 200 && status < 300) {
      return;
    }
    throw Exception(response.data?['message'] ?? 'Could not register device');
  }

  /// Unregisters a push device token. Not yet wired into any flow.
  Future<void> unregisterDevice(String token) async {
    final response = await apiClient.post(
      '/api/auth/Devices/unregister',
      data: {'token': token},
    );
    final status = response.statusCode ?? 0;
    if (status >= 200 && status < 300) {
      return;
    }
    throw Exception(response.data?['message'] ?? 'Could not unregister device');
  }

  Future<bool> isLoggedIn() async {
    if (kIsWeb) {
      try {
        _log.fine('Checking logged-in status via session check');
        return await _performSessionCheck();
      } catch (e) {
        _log.warning('isLoggedIn check failed after interceptor attempt', e);
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

  Future<String?> getDisplayName() async {
    final prefs = await SharedPreferences.getInstance();
    return prefs.getString('displayName');
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