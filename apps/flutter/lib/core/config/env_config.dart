// ==== FILE: /home/sigmund/Documents/projects/map-app/lib/data/env_config.dart ====
import 'package:flutter/foundation.dart';

enum Environment {
  development,
  production,
}

class EnvironmentConfig {
  static Environment get currentEnvironment =>
      kReleaseMode ? Environment.production : Environment.development;

  // Check if we're in development mode
  static bool get isDevelopment => currentEnvironment == Environment.development;

  // Optional override used by integration tests / dev builds pointing at a
  // local compose stack: `--dart-define=API_BASE_URL=http://localhost:8080`.
  static const String _apiBaseUrlOverride =
      String.fromEnvironment('API_BASE_URL');

  // API Base URL based on environment
  static String get apiBaseUrl {
    if (_apiBaseUrlOverride.isNotEmpty) {
      return _apiBaseUrlOverride;
    }
    if (isDevelopment) {
      // For local development
      if (kIsWeb) {
        return 'http://localhost:5000';
      } else {
        // For mobile emulators
        return 'http://10.0.2.2:5000';
      }
    } else {
      // Production environment
      return 'https://kart-api.sandring.no';
    }
  }

  /// Base URL of the hosted Flutter-web frontend. Used to build shareable
  /// links for markers and routes (e.g. `<webBaseUrl>/share/m?d=...`).
  static String get webBaseUrl {
    if (isDevelopment) {
      return 'http://localhost:8080';
    } else {
      return 'https://kart.sandring.no';
    }
  }

  static String get googleServerClientId {
    if (isDevelopment) {
      return '863382325847-l4s030g7ruif29o6no75ugb19c380an0.apps.googleusercontent.com';
    } else {
      return '863382325847-qahghop28qnml8k9vjf85dsp6o9g7mj5.apps.googleusercontent.com';
    }
  }
}