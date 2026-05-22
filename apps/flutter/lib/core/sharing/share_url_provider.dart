import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../config/env_config.dart';

/// Base URL for share links, derived from [EnvironmentConfig.webBaseUrl].
/// Exposed as a provider so tests can override it.
final webBaseUrlProvider = Provider<String>((ref) {
  return EnvironmentConfig.webBaseUrl;
});
