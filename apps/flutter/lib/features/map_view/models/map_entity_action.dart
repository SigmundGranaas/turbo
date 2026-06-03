import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:latlong2/latlong.dart';

import 'package:turbo/core/geo/geo_path.dart';

/// What the user has selected on the map and what can be done with it. The
/// capability fields ([path] / [point]) gate the journey actions; the optional
/// callbacks gate the entity-specific actions (edit/export/delete/…). An action
/// shows up iff its capability/callback is present — so markers, saved paths,
/// trails and activities all get a consistent, correct action bar without each
/// sheet hand-picking buttons.
class MapEntityActionContext {
  final WidgetRef ref;
  final BuildContext context;

  /// Display name of the entity (used as the journey label, save title, …).
  final String title;

  /// Present when the entity is a polyline (trail / saved path / route /
  /// activity route) → enables Follow / Track / Save-as-track.
  final GeoPath? path;

  /// Present when the entity is a single point (marker) → enables Navigate.
  final LatLng? point;

  // Entity-specific actions. Each is shown only when supplied.
  final VoidCallback? onSaveAsTrack;
  final VoidCallback? onAddToCollection;
  final VoidCallback? onEdit;
  final VoidCallback? onShare;
  final VoidCallback? onExport;
  final VoidCallback? onDelete;

  /// Run after a journey action fires (e.g. close the sheet).
  final VoidCallback? afterJourneyAction;

  /// When false, the canonical registry actions (Follow/Navigate/Conditions/…)
  /// are suppressed and ONLY [extraActions] are shown — i.e. this selection
  /// brings its own complete action set. Used by the coordinate selection,
  /// whose actions (Navigate-as-route, Create marker, Measure, Plan route) are
  /// coordinate-specific and shouldn't mix with the entity defaults.
  final bool includeStandardActions;

  /// Entity-specific actions appended after the standard set, so a feature can
  /// add its own buttons (e.g. a marker's "Save as activity"/"Photo") into the
  /// SAME bar instead of falling back to a bespoke action row.
  final List<MapEntityAction> extraActions;

  const MapEntityActionContext({
    required this.ref,
    required this.context,
    required this.title,
    this.path,
    this.point,
    this.onSaveAsTrack,
    this.onAddToCollection,
    this.onEdit,
    this.onShare,
    this.onExport,
    this.onDelete,
    this.afterJourneyAction,
    this.extraActions = const [],
    this.includeStandardActions = true,
  });
}

typedef MapEntityActionAvailable = bool Function(MapEntityActionContext ctx);
typedef MapEntityActionInvoke = void Function(MapEntityActionContext ctx);

/// One action a selected map entity can offer. The composition seam for
/// "what can I do with this thing" — mirrors the layer/tool/overlay registries.
class MapEntityAction {
  final String id;
  final String label;
  final IconData icon;
  final bool isDestructive;
  final MapEntityActionAvailable isAvailable;
  final MapEntityActionInvoke invoke;

  const MapEntityAction({
    required this.id,
    required this.label,
    required this.icon,
    required this.isAvailable,
    required this.invoke,
    this.isDestructive = false,
  });
}
