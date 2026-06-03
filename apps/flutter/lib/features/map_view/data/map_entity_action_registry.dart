import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:latlong2/latlong.dart';

import 'package:turbo/app/l10n/app_localizations.dart';
import 'package:turbo/core/geo/geo_path.dart';
import 'package:turbo/core/location/location_state.dart';
import 'package:turbo/core/widgets/app_dialog.dart';
import 'package:turbo/core/widgets/app_snackbars.dart';
import 'package:turbo/core/widgets/exclusive_sheet.dart';
import 'package:turbo/features/journey/api.dart';
import 'package:turbo/features/routing/api.dart';

import '../models/conditions_source.dart';
import '../models/map_entity_action.dart';
import '../widgets/conditions_sheet.dart';
import 'map_conditions_registry.dart';

/// Ordered registry of actions a selected map entity can offer. Order here is
/// display order in the action bar.
class MapEntityActionRegistry {
  final List<MapEntityAction> _all;

  MapEntityActionRegistry(Iterable<MapEntityAction> actions)
      : _all = actions.toList(growable: false);

  /// Actions available for [ctx] — the standard set in registry order, then the
  /// entity's own [MapEntityActionContext.extraActions].
  List<MapEntityAction> availableFor(MapEntityActionContext ctx) => [
        if (ctx.includeStandardActions)
          ..._all.where((a) => a.isAvailable(ctx)),
        ...ctx.extraActions.where((a) => a.isAvailable(ctx)),
      ];
}

/// The standard action set. Unlike the layer/tool/overlay registries (which
/// are build-specific compositions), the entity actions are *canonical* — every
/// build wants Follow/Track/Navigate/… — so the default is the real set, not
/// empty. Each action is gated by a capability (a `GeoPath`, a point) or an
/// entity-supplied callback, so one registry serves trails, saved paths,
/// routes, activities and markers. Override the provider in tests/builds only
/// to customise.
final mapEntityActionRegistryProvider = Provider<MapEntityActionRegistry>((ref) {
  return MapEntityActionRegistry(_standardActions);
});

final List<MapEntityAction> _standardActions = [
  MapEntityAction(
    id: 'follow',
    label: 'Follow',
    icon: Icons.directions_walk,
    isAvailable: (c) => c.path != null,
    invoke: (c) {
      c.ref
          .read(activeJourneyProvider.notifier)
          .followPath(c.path!, label: c.title);
      c.afterJourneyAction?.call();
    },
  ),
  // No separate "Track": following is the state you enter; recording is a
  // feature you toggle from inside the active-outing panel. One entry.
  MapEntityAction(
    id: 'navigate',
    label: 'Navigate',
    icon: Icons.navigation_outlined,
    isAvailable: (c) => c.point != null,
    invoke: _navigate,
  ),
  MapEntityAction(
    id: 'conditions',
    label: 'Conditions',
    icon: Icons.wb_cloudy_outlined,
    isAvailable: (c) {
      final hasPoint =
          c.point != null || (c.path != null && c.path!.points.isNotEmpty);
      return hasPoint && c.ref.read(mapConditionsRegistryProvider).isNotEmpty;
    },
    invoke: _openConditions,
  ),
  MapEntityAction(
    id: 'save_as_track',
    label: 'Save',
    icon: Icons.bookmark_add_outlined,
    isAvailable: (c) => c.onSaveAsTrack != null,
    invoke: (c) => c.onSaveAsTrack?.call(),
  ),
  MapEntityAction(
    id: 'add_to_collection',
    label: 'Collection',
    icon: Icons.collections_bookmark_outlined,
    isAvailable: (c) => c.onAddToCollection != null,
    invoke: (c) => c.onAddToCollection?.call(),
  ),
  MapEntityAction(
    id: 'edit',
    label: 'Edit',
    icon: Icons.edit_outlined,
    isAvailable: (c) => c.onEdit != null,
    invoke: (c) => c.onEdit?.call(),
  ),
  MapEntityAction(
    id: 'share',
    label: 'Share',
    icon: Icons.ios_share_outlined,
    isAvailable: (c) => c.onShare != null,
    invoke: (c) => c.onShare?.call(),
  ),
  MapEntityAction(
    id: 'export',
    label: 'Export',
    icon: Icons.download_outlined,
    isAvailable: (c) => c.onExport != null,
    invoke: (c) => c.onExport?.call(),
  ),
  MapEntityAction(
    id: 'delete',
    label: 'Delete',
    icon: Icons.delete_outline,
    isDestructive: true,
    isAvailable: (c) => c.onDelete != null,
    invoke: (c) => c.onDelete?.call(),
  ),
];

/// Navigate to the entity's point. Solves a real walking route from the user's
/// current location and follows it — the straight-line "as the crow flies"
/// point navigation is retired (it made no sense for hiking). Only when there's
/// no fix to route *from* do we fall back to heading straight there. Centralises
/// the "already navigating → confirm replace / same target → no-op" behaviour
/// so every Navigate action behaves consistently.
Future<void> _navigate(MapEntityActionContext c) async {
  final point = c.point;
  if (point == null) return;
  final notifier = c.ref.read(activeJourneyProvider.notifier);
  final journey = c.ref.read(activeJourneyProvider);
  final l10n = c.context.l10n;

  if (journey.isActive && journey.destination == point) {
    AppSnackbars.info(c.context, l10n.alreadyNavigatingHere);
    return;
  }
  if (journey.isActive) {
    final confirmed = await AppDialog.confirm(
      c.context,
      title: l10n.replaceNavigationTitle,
      content: l10n.replaceNavigationMessage,
      confirmLabel: l10n.replace,
    );
    if (!confirmed || !c.context.mounted) return;
  }

  final me = c.ref.read(locationStateProvider).value;
  if (me == null) {
    // No start to route from — head straight there as a last resort.
    notifier.navigateToPoint(point, label: c.title);
    c.afterJourneyAction?.call();
    return;
  }
  try {
    final plan = await c.ref
        .read(routingRepositoryProvider)
        .plan(points: [me, point]);
    if (!c.context.mounted) return;
    notifier.followPath(plan.toGeoPath(),
        label: c.title, waypoints: [me, point]);
    c.afterJourneyAction?.call();
  } catch (_) {
    if (!c.context.mounted) return;
    notifier.navigateToPoint(point, label: c.title);
    c.afterJourneyAction?.call();
    AppSnackbars.error(
        c.context, 'Could not plan a route — heading straight there.');
  }
}

LatLng? _midpoint(GeoPath? path) {
  if (path == null || path.points.isEmpty) return null;
  return path.points[path.points.length ~/ 2];
}

/// Open the conditions chooser for the entity's point (or a path's midpoint),
/// then show the chosen source's detail. Skips the chooser with one source.
Future<void> _openConditions(MapEntityActionContext c) async {
  final registry = c.ref.read(mapConditionsRegistryProvider);
  if (registry.sources.isEmpty) return;
  final point = c.point ?? _midpoint(c.path);
  if (point == null) return;

  ConditionsSource? chosen;
  if (registry.sources.length == 1) {
    chosen = registry.sources.first;
  } else {
    chosen = await showExclusiveSheet<ConditionsSource>(
      c.context,
      builder: (_) => ConditionsSheet(sources: registry.sources, point: point),
    );
  }
  if (chosen != null && c.context.mounted) {
    await chosen.show(c.context, point);
  }
}
