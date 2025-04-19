import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter/foundation.dart';
import 'package:http/http.dart' as http;
import 'dart:convert';
import 'auth_service.dart';

final authServiceProvider = Provider<AuthService>((ref) {
  // Use production URL for all environments
  const String baseUrl = 'https://kart-api.sandring.no';

  return AuthService(baseUrl: baseUrl);
});

final authStateProvider = StateNotifierProvider<AuthStateNotifier, AuthState>((ref) {
  return AuthStateNotifier(ref.watch(authServiceProvider));
});

enum AuthStatus { initial, loading, authenticated, unauthenticated }

class AuthState {
  final AuthStatus status;
  final String? email;
  final String? errorMessage;
  final bool isGoogleUser;
  final String? accessToken;
  final String? refreshToken;

  AuthState({
    this.status = AuthStatus.initial,
    this.email,
    this.errorMessage,
    this.isGoogleUser = false,
    this.accessToken,
    this.refreshToken,
  });

  AuthState copyWith({
    AuthStatus? status,
    String? email,
    String? errorMessage,
    bool? isGoogleUser,
    String? accessToken,
    String? refreshToken,
  }) {
    return AuthState(
      status: status ?? this.status,
      email: email ?? this.email,
      errorMessage: errorMessage,  // Not using ?? to allow setting to null
      isGoogleUser: isGoogleUser ?? this.isGoogleUser,
      accessToken: accessToken ?? this.accessToken,
      refreshToken: refreshToken ?? this.refreshToken,
    );
  }

  // Method to clear error message
  AuthState clearError() {
    return AuthState(
      status: status,
      email: email,
      errorMessage: null,
      isGoogleUser: isGoogleUser,
      accessToken: accessToken,
      refreshToken: refreshToken,
    );
  }
}

class AuthStateNotifier extends StateNotifier<AuthState> {
  final AuthService _authService;

  AuthStateNotifier(this._authService) : super(AuthState());

  Future<void> initialize() async {
    try {
      final isLoggedIn = await _authService.isLoggedIn();
      if (isLoggedIn) {
        final email = await _authService.getUserEmail();
        final isGoogleUser = await _authService.isGoogleUser();
        final accessToken = await _authService.getAccessToken();

        state = state.copyWith(
          status: AuthStatus.authenticated,
          email: email,
          isGoogleUser: isGoogleUser,
          accessToken: accessToken,
        );

        if (kDebugMode) {
          print('Initialized auth state: authenticated as $email (Google: $isGoogleUser)');
        }
      } else {
        state = state.copyWith(status: AuthStatus.unauthenticated);
        if (kDebugMode) {
          print('Initialized auth state: unauthenticated');
        }
      }
    } catch (e) {
      if (kDebugMode) {
        print('Error initializing auth state: $e');
      }
      state = state.copyWith(
          status: AuthStatus.unauthenticated,
          errorMessage: 'Failed to initialize: $e'
      );
    }
  }

  Future<void> login(String email, String password) async {
    // Clear previous errors and set loading state
    state = state.copyWith(
        status: AuthStatus.loading,
        errorMessage: null
    );

    try {
      final success = await _authService.login(email, password);
      if (success) {
        final accessToken = await _authService.getAccessToken();

        state = state.copyWith(
          status: AuthStatus.authenticated,
          email: email,
          isGoogleUser: false,
          accessToken: accessToken,
        );

        if (kDebugMode) {
          print('Login successful for $email');
        }
      }
    } catch (e) {
      if (kDebugMode) {
        print('Login error: $e');
      }
      state = state.copyWith(
        status: AuthStatus.unauthenticated,
        errorMessage: e.toString(),
      );
    }
  }

  Future<void> register(String email, String password) async {
    // Clear previous errors and set loading state
    state = state.copyWith(
        status: AuthStatus.loading,
        errorMessage: null
    );

    try {
      final success = await _authService.register(email, password, password);
      if (success) {
        final accessToken = await _authService.getAccessToken();

        state = state.copyWith(
          status: AuthStatus.authenticated,
          email: email,
          isGoogleUser: false,
          accessToken: accessToken,
        );

        if (kDebugMode) {
          print('Registration successful for $email');
        }
      }
    } catch (e) {
      if (kDebugMode) {
        print('Registration error: $e');
      }
      state = state.copyWith(
        status: AuthStatus.unauthenticated,
        errorMessage: e.toString(),
      );
    }
  }

  Future<void> loginWithGoogle(String idToken) async {
    // Clear previous errors and set loading state
    state = state.copyWith(
        status: AuthStatus.loading,
        errorMessage: null
    );

    try {
      final success = await _authService.loginWithGoogle(idToken);
      if (success) {
        final email = await _authService.getUserEmail();
        final accessToken = await _authService.getAccessToken();

        state = state.copyWith(
          status: AuthStatus.authenticated,
          email: email,
          isGoogleUser: true,
          accessToken: accessToken,
        );

        if (kDebugMode) {
          print('Google login successful for $email');
        }
      }
    } catch (e) {
      if (kDebugMode) {
        print('Google login error: $e');
      }
      state = state.copyWith(
        status: AuthStatus.unauthenticated,
        errorMessage: e.toString(),
      );
    }
  }

  Future<String> getGoogleAuthUrl() async {
    try {
      final url = await _authService.getGoogleAuthUrl();
      if (kDebugMode) {
        print('Got Google auth URL: $url');
      }
      return url;
    } catch (e) {
      // Set error but don't change auth state
      state = state.copyWith(
        errorMessage: "Couldn't get Google auth URL: ${e.toString()}",
      );
      rethrow;
    }
  }

  // Process OAuth callback (used by deep links and web callbacks)
  Future<void> processOAuthCallback(String code) async {
    if (kDebugMode) {
      print('Processing OAuth callback with code: $code');
    }

    state = state.copyWith(
        status: AuthStatus.loading,
        errorMessage: null
    );

    try {
      // For Flutter, we need to make a direct API call to exchange the code
      final baseUrl = _authService.baseUrl;
      final response = await http.get(
        Uri.parse('$baseUrl/api/auth/google/callback?code=$code'),
        // Use this instead for browser-based apps
        headers: {'Accept': 'application/json'},
      );

      // Success should be indicated by cookies being set
      // We now need to check if we're authenticated
      final isLoggedIn = await _authService.isLoggedIn();

      if (isLoggedIn) {
        final email = await _authService.getUserEmail();
        final accessToken = await _authService.getAccessToken();

        state = state.copyWith(
          status: AuthStatus.authenticated,
          email: email,
          isGoogleUser: true,
          accessToken: accessToken,
        );

        if (kDebugMode) {
          print('OAuth callback successful for $email');
        }
      } else {
        String errorMessage = 'Authentication failed';

        // Try to extract error from response
        if (response.statusCode != 200) {
          try {
            final data = jsonDecode(response.body);
            errorMessage = data['error'] ?? errorMessage;
          } catch (_) {
            // Ignore JSON parsing errors
          }
        }

        state = state.copyWith(
          status: AuthStatus.unauthenticated,
          errorMessage: errorMessage,
        );

        if (kDebugMode) {
          print('OAuth callback failed: $errorMessage');
        }
      }
    } catch (e) {
      if (kDebugMode) {
        print('OAuth callback error: $e');
      }
      state = state.copyWith(
        status: AuthStatus.unauthenticated,
        errorMessage: e.toString(),
      );
    }
  }

  Future<void> logout() async {
    try {
      await _authService.logout();
      state = state.copyWith(
        status: AuthStatus.unauthenticated,
        email: null,
        isGoogleUser: false,
        accessToken: null,
        refreshToken: null,
      );

      if (kDebugMode) {
        print('Logout successful');
      }
    } catch (e) {
      if (kDebugMode) {
        print('Logout error: $e');
      }
      state = state.copyWith(
        errorMessage: e.toString(),
      );
    }
  }

  Future<void> refreshToken() async {
    try {
      final success = await _authService.refreshToken();
      if (!success) {
        await logout();
      } else {
        // Update the access token in state
        final accessToken = await _authService.getAccessToken();
        state = state.copyWith(accessToken: accessToken);

        if (kDebugMode) {
          print('Token refresh successful');
        }
      }
    } catch (e) {
      if (kDebugMode) {
        print('Token refresh error: $e');
      }
      await logout();
    }
  }

  void clearErrors() {
    state = state.clearError();
  }
}