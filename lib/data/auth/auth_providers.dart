import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter/foundation.dart';
import 'package:http/http.dart' as http;
import 'dart:convert';
import 'package:uni_links/uni_links.dart';
import '../api_client.dart';
import 'auth_init_provider.dart';
import 'auth_service.dart';
import '../env_config.dart';


final authServiceProvider = Provider<AuthService>((ref) {
  return AuthService();
});

final authenticatedApiClientProvider = Provider<ApiClient>((ref) {
  final authService = ref.watch(authServiceProvider);
  return authService.client;
});

final authStateProvider = StateNotifierProvider<AuthStateNotifier, AuthState>((ref) {
  return AuthStateNotifier(ref.watch(authServiceProvider), ref);
});

enum AuthStatus { initial, loading, authenticated, unauthenticated, error }

class AuthState {
  final AuthStatus status;
  final String? email;
  final String? errorMessage;
  final bool isGoogleUser;
  final String? accessToken;

  AuthState({
    this.status = AuthStatus.initial,
    this.email,
    this.errorMessage,
    this.isGoogleUser = false,
    this.accessToken,
  });

  AuthState copyWith({
    AuthStatus? status,
    String? email,
    String? errorMessage,
    bool? removeError,
    bool? isGoogleUser,
    String? accessToken,
  }) {
    return AuthState(
      status: status ?? this.status,
      email: email ?? this.email,
      errorMessage: removeError == true ? null : errorMessage ?? this.errorMessage,
      isGoogleUser: isGoogleUser ?? this.isGoogleUser,
      accessToken: accessToken ?? this.accessToken,
    );
  }
}

class AuthStateNotifier extends StateNotifier<AuthState> {
  final AuthService _authService;
  // ignore: unused_field
  final Ref _ref; // Keep Ref if needed for other purposes, though not for link stream here
  AuthStatus? _previousStatus;

  AuthStatus? get previousAuthStatus => _previousStatus;

  AuthStateNotifier(this._authService, this._ref) : super(AuthState());

  void _updateState(AuthState newState) {
    _previousStatus = state.status;
    state = newState;
  }

  Future<void> initializeAndHandleInitialLink() async {
    _updateState(state.copyWith(status: AuthStatus.loading, removeError: true));
    try {
      final isLoggedIn = await _authService.isLoggedIn();
      if (isLoggedIn) {
        final email = await _authService.getUserEmail();
        final isGoogleUser = await _authService.isGoogleUser();
        final token = await _authService.getAccessToken();
        _updateState(AuthState(
          status: AuthStatus.authenticated,
          email: email,
          isGoogleUser: isGoogleUser,
          accessToken: token,
        ));
      } else {
        _updateState(AuthState(status: AuthStatus.unauthenticated));
      }

      // Handle initial link/callback
      if (kIsWeb) {
        final uri = Uri.base;
        if (uri.path.contains('/login/callback')) {
          if (kDebugMode) print('AuthStateNotifier: Detected initial web callback URL: $uri');
          final code = uri.queryParameters['code'];
          if (code != null) {
            // This call to processOAuthCallback will update the state internally.
            await processOAuthCallback(code);
          }
        }
      } else { // Mobile
        try {
          final initialLink = await getInitialLink();
          if (initialLink != null) {
            if (kDebugMode) print('AuthStateNotifier: Handling initial mobile deep link: $initialLink');
            // Use the shared helper. 'this' is the AuthStateNotifier instance.
            handleDeepLinkForProvider(initialLink, this);
          }
        } catch (e) {
          if (kDebugMode) print('AuthStateNotifier: Error handling initial deep link: $e');
        }
      }
    } catch (e) {
      _updateState(AuthState(status: AuthStatus.error, errorMessage: 'Initialization failed: $e'));
    }
  }


  Future<void> login(String email, String password) async {
    _updateState(state.copyWith(status: AuthStatus.loading, removeError: true));
    try {
      await _authService.login(email, password);
      final token = await _authService.getAccessToken();
      _updateState(AuthState(
        status: AuthStatus.authenticated,
        email: email,
        isGoogleUser: false,
        accessToken: token,
      ));
    } catch (e) {
      _updateState(state.copyWith(status: AuthStatus.unauthenticated, errorMessage: e.toString()));
    }
  }

  Future<void> register(String email, String password) async {
    _updateState(state.copyWith(status: AuthStatus.loading, removeError: true));
    try {
      await _authService.register(email, password, password);
      final token = await _authService.getAccessToken();
      _updateState(AuthState(
        status: AuthStatus.authenticated,
        email: email,
        isGoogleUser: false,
        accessToken: token,
      ));
    } catch (e) {
      _updateState(state.copyWith(status: AuthStatus.unauthenticated, errorMessage: e.toString()));
    }
  }

  Future<void> loginWithGoogle(String idToken) async {
    _updateState(state.copyWith(status: AuthStatus.loading, removeError: true));
    try {
      await _authService.loginWithGoogle(idToken);
      final email = await _authService.getUserEmail();
      final token = await _authService.getAccessToken();
      _updateState(AuthState(
        status: AuthStatus.authenticated,
        email: email,
        isGoogleUser: true,
        accessToken: token,
      ));
    } catch (e) {
      _updateState(state.copyWith(status: AuthStatus.unauthenticated, errorMessage: e.toString()));
    }
  }

  Future<String> getGoogleAuthUrl() async {
    try {
      return await _authService.getGoogleAuthUrl();
    } catch (e) {
      _updateState(state.copyWith(errorMessage: "Couldn't get Google auth URL: ${e.toString()}"));
      rethrow;
    }
  }

  Future<void> processOAuthCallback(String code) async {
    // Only set loading if not already authenticated to avoid UI flicker if already logged in by token.
    if (state.status != AuthStatus.authenticated) {
      _updateState(state.copyWith(status: AuthStatus.loading, removeError: true));
    }
    try {
      final response = await http.get(
        Uri.parse('${EnvironmentConfig.apiBaseUrl}/api/auth/google/callback?code=$code'),
        headers: {'Accept': 'application/json'},
      );

      if (response.statusCode >= 200 && response.statusCode < 300 ) {
        // After successful callback, rely on AuthService to update tokens/cookies
        // Then, re-check login status via service
        final isLoggedIn = await _authService.isLoggedIn(); // This should now reflect the callback's effect
        if(isLoggedIn) {
          final email = await _authService.getUserEmail();
          final token = await _authService.getAccessToken(); // For mobile
          _updateState(AuthState(
            status: AuthStatus.authenticated,
            email: email,
            isGoogleUser: true, // Assume Google if through this flow
            accessToken: token,
          ));
        } else {
          // This case might happen if cookie/token setting failed post-callback.
          throw Exception('OAuth callback processed but not logged in via service state.');
        }
      } else {
        String errorMessage = 'Authentication failed during OAuth callback';
        try {
          final data = jsonDecode(response.body);
          errorMessage = data['error'] ?? data['detail'] ?? errorMessage;
        } catch (_) {
          // Use response body if JSON decoding fails
          errorMessage = response.body.isNotEmpty ? response.body : errorMessage;
        }
        throw Exception(errorMessage);
      }
    } catch (e) {
      _updateState(state.copyWith(status: AuthStatus.unauthenticated, errorMessage: e.toString()));
    }
  }

  Future<void> logout() async {
    _updateState(state.copyWith(status: AuthStatus.loading));
    try {
      await _authService.logout();
      _updateState(AuthState(status: AuthStatus.unauthenticated));
    } catch (e) {
      _updateState(AuthState(status: AuthStatus.unauthenticated, errorMessage: "Logout failed: $e (still logged out locally)"));
    }
  }

  Future<void> refreshTokenAndUpdateState() async {
    if (kIsWeb) return;
    if (state.status != AuthStatus.authenticated) return;

    try {
      final success = await _authService.refreshToken();
      if (!success) {
        await logout(); // Full logout if refresh fails
      } else {
        final token = await _authService.getAccessToken();
        _updateState(state.copyWith(accessToken: token));
      }
    } catch (e) {
      // Ensure logout on any error during refresh
      await logout();
    }
  }

  void clearErrors() {
    _updateState(state.copyWith(removeError: true));
  }
}