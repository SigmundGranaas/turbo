import 'dart:async';

import 'package:flutter/foundation.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import 'package:turbo/app/app.dart';
import 'package:turbo/core/data/database_provider.dart';
import 'package:turbo/core/service/logger.dart';
import 'package:turbo/features/auth/api.dart';
import 'package:turbo/features/markers/api.dart';
import 'package:turbo/features/tile_storage/offline_regions/api.dart'
    as offline_regions;

void main() {
  WidgetsFlutterBinding.ensureInitialized();
  setupLogging();

  final container = ProviderContainer();
  unawaited(_kickOffBackgroundInit(container));

  runApp(
    UncontrolledProviderScope(
      container: container,
      child: const TurboApp(),
    ),
  );
}

/// Triggers async feature initializers without blocking the first frame.
///
/// Each read/listen starts a provider's `build()`; the UI renders in parallel
/// using the providers' loading states.
Future<void> _kickOffBackgroundInit(ProviderContainer container) async {
  if (!kIsWeb) {
    container.read(databaseProvider);
    // Listen rather than read so the orchestrator starts as soon as its async
    // dependencies resolve.
    container.listen(offline_regions.downloadOrchestratorProvider, (_, _) {});
  }
  container.read(authStateProvider);
  container.read(localMarkerDataStoreProvider);
}
