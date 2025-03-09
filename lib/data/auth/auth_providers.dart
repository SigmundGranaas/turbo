import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'auth_service.dart';

final authServiceProvider = Provider<AuthService>((ref) {
  return AuthService(baseUrl: 'http://localhost:5001');
});

final authStateProvider = StateNotifierProvider<AuthStateNotifier, AuthState>((ref) {
  return AuthStateNotifier(ref.watch(authServiceProvider));
});

enum AuthStatus { initial, authenticated, unauthenticated }

class AuthState {
  final AuthStatus status;
  final String? email;
  final String? errorMessage;

  AuthState({
    this.status = AuthStatus.initial,
    this.email,
    this.errorMessage,
  });

  AuthState copyWith({
    AuthStatus? status,
    String? email,
    String? errorMessage,
  }) {
    return AuthState(
      status: status ?? this.status,
      email: email ?? this.email,
      errorMessage: errorMessage ?? this.errorMessage,
    );
  }

  // Add this method to clear error message
  AuthState clearError() {
    return AuthState(
      status: status,
      email: email,
      errorMessage: null,
    );
  }
}

class AuthStateNotifier extends StateNotifier<AuthState> {
  final AuthService _authService;

  AuthStateNotifier(this._authService) : super(AuthState()) {
    _initialize();
  }

  Future<void> _initialize() async {
    final isLoggedIn = await _authService.isLoggedIn();
    if (isLoggedIn) {
      final email = await _authService.getUserEmail();
      state = state.copyWith(status: AuthStatus.authenticated, email: email);
    } else {
      state = state.copyWith(status: AuthStatus.unauthenticated);
    }
  }

  Future<void> login(String email, String password) async {
    // Clear any previous error messages
    state = state.clearError();

    try {
      final success = await _authService.login(email, password);
      if (success) {
        state = state.copyWith(
          status: AuthStatus.authenticated,
          email: email,
        );
      }
    } catch (e) {
      state = state.copyWith(
        status: AuthStatus.unauthenticated,
        errorMessage: e.toString(),
      );
    }
  }

  Future<void> register(String email, String password, String confirmPassword) async {
    // Clear any previous error messages
    state = state.clearError();

    try {
      final success = await _authService.register(email, password, confirmPassword);
      if (success) {
        await login(email, password);
      }
    } catch (e) {
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
      );
    } catch (e) {
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
      }
    } catch (e) {
      await logout();
    }
  }

  void clearErrors() {
    state = state.clearError();
  }
}