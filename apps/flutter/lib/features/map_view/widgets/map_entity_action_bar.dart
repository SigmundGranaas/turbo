import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import 'package:turbo/core/widgets/sheet_action_bar.dart';

import '../data/map_entity_action_registry.dart';
import '../models/map_entity_action.dart';

/// Renders the registry's available actions for a selected map entity, using
/// the same adaptive [SheetActionBar] chrome every detail sheet already uses.
/// Drop this into any entity sheet with a [MapEntityActionContext]; the right
/// buttons (Follow / Track / Navigate / Save / … ) appear based on the
/// entity's capabilities — no per-sheet button picking.
class MapEntityActionBar extends ConsumerWidget {
  final MapEntityActionContext entity;
  final int maxInline;

  const MapEntityActionBar({
    super.key,
    required this.entity,
    this.maxInline = 4,
  });

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final actions =
        ref.watch(mapEntityActionRegistryProvider).availableFor(entity);
    if (actions.isEmpty) return const SizedBox.shrink();
    return SheetActionBar(
      maxInline: maxInline,
      actions: [
        for (final a in actions)
          SheetAction(
            icon: a.icon,
            label: a.label,
            isDestructive: a.isDestructive,
            onPressed: () => a.invoke(entity),
          ),
      ],
    );
  }
}
