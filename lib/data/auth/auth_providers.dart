import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter/foundation.dart';
import 'package:uni_links/uni_links.dart';
import '../api_client.dart';
import 'auth_init_provider.dart';
import 'auth_service.dart';


final authServiceProvider = Provider<AuthService>((ref) {
  return AuthService();
});

final authenticatedApiClientProvider = Provider<ApiClient>((ref) {
  final authService = ref.watch(authServiceProvider);
  return authService.client;
});

final authStateProvider = StateNotifierProvider<AuthStateNotifier, AuthState>((ref) {
  return AuthStateNotifier(ref.watch(authServiceProvider), ref.read(authenticatedApiClientProvider), ref);
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
  final ApiClient _apiClient;
  // ignore: unused_field
  final Ref _ref;
  AuthStatus? _previousStatus;

  AuthStatus? get previousAuthStatus => _previousStatus;

  AuthStateNotifier(this._authService, this._apiClient, this._ref) : super(AuthState());

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
            await processOAuthCallback(code);
          }
        }
      } else { // Mobile
        try {
          final initialLink = await getInitialLink();
          if (initialLink != null) {
            if (kDebugMode) print('AuthStateNotifier: Handling initial mobile deep link: $initialLink');
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

  Future<String> getGoogleAuthUrl() async {
    try {
      return await _authService.getGoogleAuthUrl();
    } catch (e) {
      _updateState(state.copyWith(errorMessage: "Couldn't get Google auth URL: ${e.toString()}"));
      rethrow;
    }
  }

  Future<void> processOAuthCallback(String code) async {
    if (state.status != AuthStatus.authenticated) {
      _updateState(state.copyWith(status: AuthStatus.loading, removeError: true));
    }

    try {
      final response = await _apiClient.get('/api/v1/OAuth/google/callback', queryParameters: {'code': code});

      if (response.statusCode == 200) {
        if (kIsWeb) {
          // On web, cookies are set by the browser. We just need to confirm the session.
          final sessionIsValid = await _authService.isLoggedIn(); // This calls /me
          if (sessionIsValid) {
            final email = await _authService.getUserEmail();
            await _authService.setIsGoogleUser(true); // Manually set flag
            _updateState(AuthState(
              status: AuthStatus.authenticated,
              email: email,
              isGoogleUser: true,
            ));
          } else {
            throw Exception('OAuth callback processed but session is not valid.');
          }
        } else { // Mobile
          final data = response.data;
          final String accessToken = data['accessToken'];
          final String refreshToken = data['refreshToken'];
          final String email = data['email'];

          await _authService.storeTokens(accessToken, refreshToken);
          await _authService.setLoginState(email: email, isGoogleUser: true);

          _updateState(AuthState(
            status: AuthStatus.authenticated,
            email: email,
            isGoogleUser: true,
            accessToken: accessToken,
          ));
        }
      } else {
        String errorMessage = 'Authentication failed during OAuth callback';
        try {
          final data = response.data;
          errorMessage = data['message'] ?? errorMessage;
        } catch (_) {
          errorMessage = response.data?.toString() ?? errorMessage;
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