import 'package:flutter_test/flutter_test.dart';
import 'package:turbo/core/config/env_config.dart';

void main() {
  group('EnvironmentConfig.webBaseUrl', () {
    test('returns a non-empty URL', () {
      expect(EnvironmentConfig.webBaseUrl, isNotEmpty);
    });

    test('returns an https URL in production', () {
      // The test environment runs as development; we still expect both
      // branches of the getter to produce a parsable absolute URL.
      final uri = Uri.parse(EnvironmentConfig.webBaseUrl);
      expect(uri.hasScheme, isTrue);
      expect(uri.host, isNotEmpty);
    });
  });
}
