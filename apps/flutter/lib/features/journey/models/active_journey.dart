import 'package:latlong2/latlong.dart';
import 'package:turbo/core/geo/geo_path.dart';

/// What the user is actively doing with their live position.
enum JourneyKind {
  /// Nothing live — the map is just a map.
  none,

  /// Heading to a single point (the old point-to-point "navigate to here").
  navigatingToPoint,

  /// Following a known polyline — a curated trail, a saved path, or a route
  /// the user just planned. This is the unifying case the old codebase lacked.
  followingPath,
}

/// The single source of truth for "an active journey".
///
/// Replaces the disconnected navigation / follow / recording silos with one
/// composable state object. Features read it to render live progress and to
/// offer contextual actions; the [ActiveJourneyNotifier] orchestrates the
/// underlying primitives (map-follow, recording) so they no longer each poke
/// follow-mode independently. See
/// `docs/architecture/2026-06-composition-overhaul-plan.md` (Phase 2).
class ActiveJourney {
  final JourneyKind kind;

  /// Destination point for [JourneyKind.navigatingToPoint].
  final LatLng? target;

  /// The path being followed for [JourneyKind.followingPath].
  final GeoPath? path;

  /// Whether a GPS recording was started as part of this journey. The
  /// recording itself lives in the path_recording feature; this only records
  /// that the journey kicked it off.
  final bool recording;

  /// Human label for the thing being followed (trail/route/path name), shown
  /// in the journey chip. Null for ad-hoc point navigation.
  final String? label;

  /// The waypoints a *route* was planned from, when the followed path is an
  /// editable route (planned route / "navigate here"). Lets the active-outing
  /// panel re-open the planner to edit the route mid-follow. Null for trails /
  /// saved paths / recordings (no editable source).
  final List<LatLng>? waypoints;

  const ActiveJourney({
    this.kind = JourneyKind.none,
    this.target,
    this.path,
    this.recording = false,
    this.label,
    this.waypoints,
  });

  static const ActiveJourney inactive = ActiveJourney();

  bool get isActive => kind != JourneyKind.none;

  /// The point the journey is ultimately heading toward — the explicit target
  /// for point navigation, or the last vertex of a followed path.
  LatLng? get destination {
    switch (kind) {
      case JourneyKind.navigatingToPoint:
        return target;
      case JourneyKind.followingPath:
        return (path != null && path!.points.isNotEmpty)
            ? path!.points.last
            : null;
      case JourneyKind.none:
        return null;
    }
  }

  ActiveJourney copyWith({
    JourneyKind? kind,
    LatLng? target,
    GeoPath? path,
    bool? recording,
    String? label,
    List<LatLng>? waypoints,
  }) {
    return ActiveJourney(
      kind: kind ?? this.kind,
      target: target ?? this.target,
      path: path ?? this.path,
      recording: recording ?? this.recording,
      label: label ?? this.label,
      waypoints: waypoints ?? this.waypoints,
    );
  }
}
