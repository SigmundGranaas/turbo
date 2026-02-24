import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter/foundation.dart';
import 'package:app_links/app_links.dart';
import '../api_client.dart';
import 'auth_init_provider.dart';
import 'auth_service.dart';

// The AuthStateNotifier is now the root. It creates and configures its own client.
final authStateProvider = NotifierProvider<AuthStateNotifier, AuthState>(() {
  return AuthStateNotifier();
});

// The AuthService depends on the client that the AuthStateNotifier owns.
final authServiceProvider = Provider<AuthService>((ref) {
  final apiClient = ref.watch(authStateProvider.notifier).apiClient;
  return AuthService(apiClient: apiClient);
});

// This provider now gives other parts of the app access to the single,
// correctly configured ApiClient instance.
final authenticatedApiClientProvider = Provider<ApiClient>((ref) {
  return ref.watch(authStateProvider.notifier).apiClient;
});

enum AuthStatus { initial, loading, authenticated, unauthenticated, error }

class AuthState {
  final AuthStatus status;
  final String? email;
  final String? errorMessage;
  final bool isGoogleUser;
  final String? accessToken;
  final bool isInitializing;

  AuthState({
    this.status = AuthStatus.initial,
    this.email,
    this.errorMessage,
    this.isGoogleUser = false,
    this.accessToken,
    this.isInitializing = false,
  });

  AuthState copyWith({
    AuthStatus? status,
    String? email,
    String? errorMessage,
    bool? removeError,
    bool? isGoogleUser,
    String? accessToken,
    bool? isInitializing,
  }) {
    return AuthState(
      status: status ?? this.status,
      email: email ?? this.email,
      errorMessage: removeError == true ? null : errorMessage ?? this.errorMessage,
      isGoogleUser: isGoogleUser ?? this.isGoogleUser,
      accessToken: accessToken ?? this.accessToken,
      isInitializing: isInitializing ?? this.isInitializing,
    );
  }
}

class AuthStateNotifier extends Notifier<AuthState> {
  late final ApiClient _apiClient;
  late final AuthService _authService;
  AuthStatus? _previousStatus;

  // Public getter for the configured ApiClient
  ApiClient get apiClient => _apiClient;
  AuthStatus? get previousAuthStatus => _previousStatus;

  @override
  AuthState build() {
    // 1. Create the ApiClient instance.
    _apiClient = ApiClient();
    // 2. Set its failure handler to call a method on this notifier instance.
    _apiClient.setAuthFailureHandler(handleAuthFailure);
    // 3. Create the AuthService using the now-configured client.
    _authService = AuthService(apiClient: _apiClient);

    // 4. Trigger background initialization without blocking.
    Future.microtask(() => initializeAndHandleInitialLink());

    // 5. Start in a guest-friendly state immediately.
    return AuthState(status: AuthStatus.unauthenticated, isInitializing: true);
  }

  /// This callback is invoked by the ApiClient when a token refresh fails.
  void handleAuthFailure() {
    // Defer the logout call to prevent "modifying state during build" errors.
    Future.microtask(() => logout());
  }

  void _updateState(AuthState newState) {
    _previousStatus = state.status;
    state = newState;
  }

  Future<void> initializeAndHandleInitialLink() async {
    try {
      if (!kIsWeb) {
        try {
          final appLinks = AppLinks();
          final initialUri = await appLinks.getInitialLink().timeout(const Duration(seconds: 2));
          if (initialUri != null) {
            handleDeepLinkForProvider(initialUri.toString(), this);
          }
        } catch (e) {
          if (kDebugMode) print('AuthStateNotifier: Background initial link check failed/timed out: $e');
        }
      }

      // Check session status with a timeout to prevent app stall
      final isLoggedIn = await _authService.isLoggedIn().timeout(
        const Duration(seconds: 5),
        onTimeout: () {
          if (kDebugMode) print('AuthStateNotifier: Session check timed out, continuing as unauthenticated.');
          return false;
        },
      );

      if (isLoggedIn) {
        final email = await _authService.getUserEmail();
        final isGoogleUser = await _authService.isGoogleUser();
        final token = await _authService.getAccessToken();
        _updateState(AuthState(
          status: AuthStatus.authenticated,
          email: email,
          isGoogleUser: isGoogleUser,
          accessToken: token,
          isInitializing: false,
        ));
      } else {
        _updateState(state.copyWith(status: AuthStatus.unauthenticated, isInitializing: false));
      }
    } catch (e) {
      if (kDebugMode) print('AuthStateNotifier: Background initialization error: $e');
      _updateState(state.copyWith(isInitializing: false));
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
    if (kIsWeb) {
      if (kDebugMode) print("processOAuthCallback called on web, which is unexpected.");
      return;
    }
    if (state.status == AuthStatus.authenticated) return;
    _updateState(state.copyWith(status: AuthStatus.loading, removeError: true));

    try {
      final response = await _apiClient.post(
        '/api/auth/OAuth/mobile-signin',
        data: {
          'provider': 'google',
          'code': code,
        },
      );

      if (response.statusCode == 200) {
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
      } else {
        String errorMessage = response.data?['message'] ?? 'Authentication failed';
        throw Exception(errorMessage);
      }
    } catch (e) {
      _updateState(state.copyWith(status: AuthStatus.unauthenticated, errorMessage: e.toString()));
    }
  }

  Future<void> logout() async {
    if (state.status == AuthStatus.loading || state.status == AuthStatus.unauthenticated) return;

    _updateState(state.copyWith(status: AuthStatus.loading));
    try {
      await _authService.logout();
    } catch (e) {
      if (kDebugMode) {
        print("Logout API call failed: $e. Logging out locally.");
      }
    } finally {
      _updateState(AuthState(status: AuthStatus.unauthenticated));
    }
  }

  Future<void> refreshTokenAndUpdateState() async {
    if (kIsWeb) return;
    if (state.status != AuthStatus.authenticated) return;

    try {
      final success = await _authService.refreshToken();
      if (!success) {
        await logout();
      } else {
        final token = await _authService.getAccessToken();
        _updateState(state.copyWith(accessToken: token));
      }
    } catch (e) {
      await logout();
    }
  }

  void clearErrors() {
    _updateState(state.copyWith(removeError: true));
  }
}