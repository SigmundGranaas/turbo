import 'package:flutter/foundation.dart';
import 'package:logging/logging.dart';

// Create a top-level logger for the application.
// Other files can access this instance directly.
final log = Logger('Turbo');

/// Sets up a global listener for the logging package.
/// This should be called once in main.dart for the app,
/// and in setUpAll for tests.
void setupLogging({Level level = Level.INFO}) {
  Logger.root.level = level;
  Logger.root.onRecord.listen((record) {
    if (kDebugMode) {
      print(
        '${record.level.name.padRight(7)}: ${record.time
            .toIso8601String()}: ${record.loggerName}: ${record.message}',
      );
    if (record.error != null) {
      print('  ERROR: ${record.error}');
    }
    if (record.stackTrace != null) {
      print('  STACKTRACE:\n${record.stackTrace}');
    }
  }
  });
}