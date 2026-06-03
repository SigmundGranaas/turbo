import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:latlong2/latlong.dart';

import 'package:turbo/core/geo/geo_metrics.dart';
import 'package:turbo/core/geo/geo_path.dart';
import 'package:turbo/core/location/follow_mode_state.dart';
import 'package:turbo/core/location/location_state.dart';
import 'package:turbo/features/path_recording/api.dart';

import '../models/active_journey.dart';

/// Orchestrates the active journey by composing existing primitives —
/// map-follow ([followModeProvider]) and recording ([recordingNotifierProvider])
/// — instead of each feature wiring them independently.
///
/// Intentionally NOT autoDispose: a journey must survive navigating between
/// screens (e.g. starting "track this route" from the planning page and
/// popping back to the map), mirroring the recording notifier.
class ActiveJourneyNotifier extends Notifier<ActiveJourney> {
  @override
  ActiveJourney build() => ActiveJourney.inactive;

  /// Head to a single point. Engages map-follow so the user sees themselves
  /// move toward the target.
  void navigateToPoint(LatLng target, {String? label}) {
    state = ActiveJourney(
      kind: JourneyKind.navigatingToPoint,
      target: target,
      label: label,
    );
    ref.read(followModeProvider.notifier).enable();
  }

  /// Follow a known polyline (trail / saved path / planned route). When
  /// [record] is true, a GPS recording is started so the outing is captured;
  /// the recording is then managed by the standard recording panel.
  Future<void> followPath(
    GeoPath path, {
    bool record = false,
    String? label,
    List<LatLng>? waypoints,
  }) async {
    state = ActiveJourney(
      kind: JourneyKind.followingPath,
      path: path,
      recording: record,
      label: label,
      waypoints: waypoints,
    );
    ref.read(followModeProvider.notifier).enable();
    if (record) {
      await ref.read(recordingNotifierProvider.notifier).start();
    }
  }

  /// Begin recording mid-follow (upgrade a Follow into a Track) without losing
  /// the followed path.
  Future<void> startRecording() async {
    if (!state.isActive) return;
    state = state.copyWith(recording: true);
    await ref.read(recordingNotifierProvider.notifier).start();
  }

  /// Swap the followed path's geometry (e.g. after an off-route re-route)
  /// WITHOUT touching any in-flight recording — so re-routing never discards
  /// the track captured so far.
  void updateFollowedPath(GeoPath path, {List<LatLng>? waypoints}) {
    if (state.kind != JourneyKind.followingPath) return;
    state = state.copyWith(path: path, waypoints: waypoints);
  }

  /// End the journey. Leaves any in-flight recording running — the recording
  /// panel owns stopping/saving it, so a user "following + recording" who ends
  /// the follow still keeps their track.
  void stop() {
    state = ActiveJourney.inactive;
    ref.read(followModeProvider.notifier).disable();
  }

  /// Live progress of the current position against the followed path. Null
  /// unless [JourneyKind.followingPath] with a known position.
  PathProgress? progressFor(LatLng position) {
    final p = state.path;
    if (state.kind != JourneyKind.followingPath || p == null) return null;
    return GeoMetrics.progress(p.points, position);
  }
}

final activeJourneyProvider =
    NotifierProvider<ActiveJourneyNotifier, ActiveJourney>(
  ActiveJourneyNotifier.new,
);

/// Live progress of the user along a *followed path*: fraction done, distance
/// remaining, ETA, and how far off-route. One source of truth for the progress
/// bar, ETA and off-route banner. Null unless following a path with a fix.
class JourneyProgress {
  final double fraction; // 0‥1 along the path
  final double remainingM;
  final double etaSeconds;
  final double offRouteM;
  const JourneyProgress({
    required this.fraction,
    required this.remainingM,
    required this.etaSeconds,
    required this.offRouteM,
  });
}

final journeyProgressProvider = Provider<JourneyProgress?>((ref) {
  final journey = ref.watch(activeJourneyProvider);
  final pos = ref.watch(locationStateProvider).value;
  if (journey.kind != JourneyKind.followingPath ||
      journey.path == null ||
      pos == null) {
    return null;
  }
  final prog = GeoMetrics.progress(journey.path!.points, pos);
  if (prog == null) return null;
  return JourneyProgress(
    fraction: prog.fraction,
    remainingM: prog.remainingM,
    etaSeconds: GeoMetrics.naismithSeconds(prog.remainingM),
    offRouteM: prog.offRouteM,
  );
});

/// Live distance-to-go for the active journey, metres. Watches both the
/// journey and the user's position so consumers (chips, layers) get one
/// derived value rather than each recomputing distance. Null when there's no
/// active journey or no fix yet.
final journeyRemainingMetersProvider = Provider<double?>((ref) {
  final journey = ref.watch(activeJourneyProvider);
  final pos = ref.watch(locationStateProvider).value;
  if (!journey.isActive || pos == null) return null;

  switch (journey.kind) {
    case JourneyKind.navigatingToPoint:
      final t = journey.target;
      return t == null ? null : GeoMetrics.distanceMeters(pos, t);
    case JourneyKind.followingPath:
      final path = journey.path;
      if (path == null) return null;
      return GeoMetrics.progress(path.points, pos)?.remainingM;
    case JourneyKind.none:
      return null;
  }
});
