import 'package:flutter/widgets.dart';
import 'package:latlong2/latlong.dart';

import 'package:turbo/core/geo/geo_path.dart';

import 'map_entity_action.dart';

/// The single "thing the user is currently inspecting on the map" — a tapped
/// coordinate, a point entity, or a path. One source of truth so any surface
/// (a long-press, a search result, a layer tap) can drive the same detail host
/// + action bar instead of each opening its own bespoke sheet.
///
/// It is essentially a serialisable source for a [MapEntityActionContext]: it
/// carries the capability data (point/path/title), the entity-specific
/// callbacks/extraActions, and an optional rich [bodyBuilder] rendered above the
/// action bar (e.g. a coordinate's place-info + weather, or a path's stats).
@immutable
class MapSelection {
  /// Present when the selection is a single point (coordinate / marker).
  final LatLng? point;

  /// Present when the selection is a polyline (trail / saved path / route).
  final GeoPath? path;

  /// Display name (journey label, save title, sheet header).
  final String title;

  /// Optional rich detail body rendered above the action bar. Null → the host
  /// shows the action bar alone.
  final WidgetBuilder? bodyBuilder;

  /// See [MapEntityActionContext.includeStandardActions].
  final bool includeStandardActions;

  final List<MapEntityAction> extraActions;
  final VoidCallback? onSaveAsTrack;
  final VoidCallback? onAddToCollection;
  final VoidCallback? onEdit;
  final VoidCallback? onShare;
  final VoidCallback? onExport;
  final VoidCallback? onDelete;

  const MapSelection({
    this.point,
    this.path,
    required this.title,
    this.bodyBuilder,
    this.includeStandardActions = true,
    this.extraActions = const [],
    this.onSaveAsTrack,
    this.onAddToCollection,
    this.onEdit,
    this.onShare,
    this.onExport,
    this.onDelete,
  });
}
