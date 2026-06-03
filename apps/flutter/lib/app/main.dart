import 'dart:async';

import 'package:flutter/foundation.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import 'package:turbo/app/app.dart';
import 'package:turbo/app/map_conditions.dart';
import 'package:turbo/app/map_layers.dart';
import 'package:turbo/app/map_overlays.dart';
import 'package:turbo/core/data/database_provider.dart';
import 'package:turbo/core/service/logger.dart';
import 'package:turbo/features/activities/api.dart' as activities;
import 'package:turbo/features/activity_backcountry_ski/api.dart'
    as activity_backcountry_ski;
import 'package:turbo/features/activity_fishing/api.dart' as activity_fishing;
import 'package:turbo/features/activity_freediving/api.dart'
    as activity_freediving;
import 'package:turbo/features/activity_hiking/api.dart' as activity_hiking;
import 'package:turbo/features/activity_packrafting/api.dart'
    as activity_packrafting;
import 'package:turbo/features/activity_xc_ski/api.dart' as activity_xc_ski;
import 'package:turbo/features/auth/api.dart';
import 'package:turbo/features/map_view/api.dart' as map_view;
import 'package:turbo/features/markers/api.dart';
import 'package:turbo/features/measuring/api.dart' as measuring;
import 'package:turbo/features/routing/api.dart' as routing;
import 'package:turbo/features/sharing/api.dart';
import 'package:turbo/features/tile_storage/offline_regions/api.dart'
    as offline_regions;

void main() {
  WidgetsFlutterBinding.ensureInitialized();
  setupLogging();

  // Compose the activity kind registry from each kind feature's descriptor.
  // Adding a kind = add its descriptor here and ship its feature module.
  // The shell never imports a specific kind feature beyond this list.
  final activityKinds = activities.ActivityKindRegistry([
    activity_fishing.fishingActivityKindDescriptor,
    activity_backcountry_ski.backcountrySkiActivityKindDescriptor,
    activity_hiking.hikingActivityKindDescriptor,
    activity_xc_ski.xcSkiActivityKindDescriptor,
    activity_packrafting.packraftingActivityKindDescriptor,
    activity_freediving.freedivingActivityKindDescriptor,
  ]);

  // Compose the map-tool registry from each tool feature's descriptor. Adding
  // a tool = add its descriptor here; the map host iterates the registry and
  // never names a specific tool.
  final mapTools = map_view.MapToolRegistry([
    routing.routePlanningTool,
    measuring.measuringTool,
    offline_regions.regionSelectTool,
  ]);

  final container = ProviderContainer(
    overrides: [
      activities.activityKindRegistryProvider.overrideWithValue(activityKinds),
      map_view.mapToolRegistryProvider.overrideWithValue(mapTools),
      map_view.mapLayerRegistryProvider
          .overrideWithValue(buildDefaultMapLayerRegistry()),
      map_view.mapOverlayRegistryProvider
          .overrideWithValue(buildDefaultMapOverlayRegistry()),
      map_view.mapConditionsRegistryProvider
          .overrideWithValue(buildDefaultMapConditionsRegistry()),
      // Wire the sharing gate to live auth status. Tests skip this override
      // and get the default (false), keeping the sharing UI hidden — and,
      // crucially, keeping the auth notifier from being instantiated as a
      // side-effect of building tested widgets.
      sharingAvailableProvider.overrideWith((ref) =>
          ref.watch(authStateProvider).status == AuthStatus.authenticated),
    ],
  );
  unawaited(_kickOffBackgroundInit(container));
  // Parse any share URL embedded in the initial location (web cold-start
  // or platform deep-link). Has to happen before the first frame so the
  // map can react.
  ShareRouteHandler(container).handle(Uri.base);

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
  // Start watching app links for share URLs (no-op on web).
  container.read(shareLinkListenerProvider);
}
