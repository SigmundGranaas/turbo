import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter/foundation.dart';
import 'package:app_links/app_links.dart';
import '../api_client.dart';
import 'auth_init_provider.dart';
import 'auth_service.dart';

// The AuthStateNotifier is now the root. It creates and configures its own client.
final authStateProvider = StateNotifierProvider<AuthStateNotifier, AuthState>((ref) {
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
  late final ApiClient _apiClient;
  late final AuthService _authService;
  AuthStatus? _previousStatus;

  // Public getter for the configured ApiClient
  ApiClient get apiClient => _apiClient;
  AuthStatus? get previousAuthStatus => _previousStatus;

  AuthStateNotifier() : super(AuthState()) {
    // 1. Create the ApiClient instance.
    _apiClient = ApiClient();
    // 2. Set its failure handler to call a method on this notifier instance.
    //    This breaks the provider circular dependency.
    _apiClient.setAuthFailureHandler(handleAuthFailure);
    // 3. Create the AuthService using the now-configured client.
    _authService = AuthService(apiClient: _apiClient);
  }

  /// This callback is invoked by the ApiClient when a token refresh fails.
  void handleAuthFailure() {
    // Defer the logout call to prevent "modifying state during build" errors.
    Future.microtask(() => logout());
  }

  void _updateState(AuthState newState) {
    if (!mounted) return;
    _previousStatus = state.status;
    state = newState;
  }

  Future<void> initializeAndHandleInitialLink() async {
    _updateState(state.copyWith(status: AuthStatus.loading, removeError: true));
    try {
      // Native Google Sign-In does not use a deep link to start the auth flow.
      // The logic for handling an initial deep link for OAuth on mobile has been removed.
      // This section can be used for other types of deep links if needed.
      if (!kIsWeb) {
        try {
          final appLinks = AppLinks();
          final initialUri = await appLinks.getInitialLink();
          if (initialUri != null) {
            // Example: Handle a non-auth deep link.
            if (kDebugMode) print('AuthStateNotifier: Handling initial deep link: ${initialUri.toString()}');
            handleDeepLinkForProvider(initialUri.toString(), this);
          }
        } catch (e) {
          if (kDebugMode) print('AuthStateNotifier: Error handling initial deep link: $e');
        }
      }

      // Now, check the session status. This is the primary method for:
      // 1. Web: After a successful OAuth redirect, the browser has the session cookie.
      // 2. Mobile/Web: On subsequent app opens to check for an existing session.
      final isLoggedIn = await _authService.isLoggedIn();
      if (isLoggedIn) {
        final email = await _authService.getUserEmail();
        final isGoogleUser = await _authService.isGoogleUser();
        final token = await _authService.getAccessToken(); // This is null on web, which is fine
        _updateState(AuthState(
          status: AuthStatus.authenticated,
          email: email,
          isGoogleUser: isGoogleUser,
          accessToken: token,
        ));
      } else {
        _updateState(AuthState(status: AuthStatus.unauthenticated));
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
    // This method handles the authorization code exchange.
    // - On mobile, it's called with the serverAuthCode from native Google Sign-In.
    // - On web, it's called by the callback page with the code from the URL.
    if (state.status == AuthStatus.authenticated) return;
    _updateState(state.copyWith(status: AuthStatus.loading, removeError: true));

    try {
      // The backend endpoint handles the code exchange.
      // For mobile (which sets Accept: application/json), it returns tokens in the body.
      // For web, it sets cookies and redirects.
      final response = await _apiClient.get('/api/auth/OAuth/google/callback', queryParameters: {'code': code});

      // The web flow is handled by the redirect and subsequent session check on page load.
      // The mobile flow needs to process the returned tokens.
      if (!kIsWeb && response.statusCode == 200) {
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
      } else if (kIsWeb) {
        // For web, a successful call means cookies are set.
        // We can re-check the session state to update the UI.
        await initializeAndHandleInitialLink();
      } else {
        // Handle API errors for mobile
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
    // Prevent multiple logout calls if already logging out or unauthenticated
    if (state.status == AuthStatus.loading || state.status == AuthStatus.unauthenticated) return;

    _updateState(state.copyWith(status: AuthStatus.loading));
    try {
      await _authService.logout();
    } catch (e) {
      // Even if logout API call fails, we still want to log out locally.
      if (kDebugMode) {
        print("Logout API call failed: $e. Logging out locally.");
      }
    } finally {
      // Ensure local state is always cleared.
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
      // Ensure logout on any error during refresh
      await logout();
    }
  }

  void clearErrors() {
    _updateState(state.copyWith(removeError: true));
  }
}