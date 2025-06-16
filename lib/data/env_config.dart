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

  // API Base URL based on environment
  static String get apiBaseUrl {
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

  static String get googleServerClientId {
    return '863382325847-l4s030g7ruif29o6no75ugb19c380an0.apps.googleusercontent.com';
  }
}