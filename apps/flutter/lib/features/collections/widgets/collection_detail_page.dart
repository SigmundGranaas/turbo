import 'package:flutter/material.dart';
import 'package:turbo/core/widgets/exclusive_sheet.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:turbo/app/l10n/app_localizations.dart';
import 'package:turbo/core/geo/geo_path.dart';
import 'package:turbo/core/widgets/app_snackbars.dart';
import 'package:turbo/features/journey/api.dart';
import 'package:turbo/features/markers/api.dart';
import 'package:turbo/features/saved_paths/api.dart';
import 'package:turbo/features/sharing/api.dart';

import '../data/collection_repository.dart';
import '../data/collection_visibility_provider.dart';
import '../models/collection.dart';
import '../models/collection_item_ref.dart';
import 'collections_page.dart' show confirmDeleteCollection;
import 'create_or_edit_collection_sheet.dart';

class CollectionDetailPage extends ConsumerWidget {
  final String collectionUuid;

  const CollectionDetailPage({super.key, required this.collectionUuid});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final l10n = context.l10n;
    final asyncState = ref.watch(collectionRepositoryProvider);

    return asyncState.when(
      loading: () => const Scaffold(
        body: Center(child: CircularProgressIndicator()),
      ),
      error: (e, _) => Scaffold(
        appBar: AppBar(),
        body: Center(child: Text('$e')),
      ),
      data: (state) {
        Collection? collection;
        for (final c in state.collections) {
          if (c.uuid == collectionUuid) {
            collection = c;
            break;
          }
        }
        if (collection == null) {
          // Collection was deleted while we were viewing it.
          return Scaffold(
            appBar: AppBar(title: Text(l10n.collection)),
            body: Center(child: Text(l10n.collectionDeleted)),
          );
        }
        return _Loaded(collection: collection, repoState: state);
      },
    );
  }
}

class _Loaded extends ConsumerStatefulWidget {
  final Collection collection;
  final CollectionRepositoryState repoState;

  const _Loaded({required this.collection, required this.repoState});

  @override
  ConsumerState<_Loaded> createState() => _LoadedState();
}

class _LoadedState extends ConsumerState<_Loaded> {
  List<CollectionItemRef> _refs = const [];
  bool _loading = true;

  @override
  void initState() {
    super.initState();
    _loadItems();
  }

  @override
  void didUpdateWidget(covariant _Loaded oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (oldWidget.collection.uuid != widget.collection.uuid ||
        oldWidget.repoState != widget.repoState) {
      _loadItems();
    }
  }

  Future<void> _loadItems() async {
    setState(() => _loading = true);
    final store = await ref.read(localCollectionDataStoreProvider.future);
    final items = await store.getItems(widget.collection.uuid);
    if (!mounted) return;
    setState(() {
      _refs = items;
      _loading = false;
    });
  }

  /// Stitch the collection's saved paths (in order) into one [GeoPath] and
  /// follow it as a single journey — "follow trip". Returns to the map so the
  /// active-outing panel takes over.
  void _followTrip(BuildContext context, List<SavedPath> tripPaths) {
    final points = [for (final p in tripPaths) ...p.points];
    if (points.length < 2) {
      AppSnackbars.info(context, 'This collection has no route to follow yet.');
      return;
    }
    ref.read(activeJourneyProvider.notifier).followPath(
          GeoPath.fromPoints(points, source: GeoPathSource.saved),
          label: widget.collection.name,
        );
    Navigator.of(context).popUntil((route) => route.isFirst);
  }

  @override
  Widget build(BuildContext context) {
    final l10n = context.l10n;
    final colorScheme = Theme.of(context).colorScheme;
    final color = hexToColor(widget.collection.colorHex) ?? colorScheme.primary;
    final iconService = IconService();
    final namedIcon = widget.collection.iconKey != null
        ? iconService.getIcon(context, widget.collection.iconKey)
        : null;
    final visibility = ref.watch(collectionVisibilityProvider);
    final isVisible = visibility[widget.collection.uuid] ?? true;

    final markerRefs =
        _refs.where((r) => r.type == CollectionItemRef.typeMarker).toList();
    final pathRefs =
        _refs.where((r) => r.type == CollectionItemRef.typePath).toList();

    final markers = ref.watch(locationRepositoryProvider).asData?.value ?? const [];
    final paths = ref.watch(savedPathRepositoryProvider).asData?.value ?? const [];
    final markerByUuid = {for (final m in markers) m.uuid: m};
    final pathByUuid = {for (final p in paths) p.uuid: p};

    // The collection's paths in insertion order — the "trip" you can follow as
    // one continuous journey.
    final tripPaths = [
      for (final r in pathRefs)
        if (pathByUuid[r.uuid] != null) pathByUuid[r.uuid]!,
    ];

    return Scaffold(
      appBar: AppBar(
        title: Text(widget.collection.name),
        actions: [
          if (tripPaths.isNotEmpty)
            IconButton(
              tooltip: 'Follow trip',
              icon: const Icon(Icons.directions_walk),
              onPressed: () => _followTrip(context, tripPaths),
            ),
          IconButton(
            tooltip: isVisible ? l10n.visibleOnMap : l10n.hiddenOnMap,
            icon: Icon(isVisible
                ? Icons.visibility_outlined
                : Icons.visibility_off_outlined),
            onPressed: () => ref
                .read(collectionVisibilityProvider.notifier)
                .toggle(widget.collection.uuid),
          ),
          IconButton(
            tooltip: l10n.edit,
            icon: const Icon(Icons.edit_outlined),
            onPressed: () => CreateOrEditCollectionSheet.show(
              context,
              existing: widget.collection,
            ),
          ),
          if (ref.watch(sharingAvailableProvider))
            IconButton(
              tooltip: 'Share',
              icon: const Icon(Icons.share_outlined),
              onPressed: () => ShareSheet.show(
                context,
                widget.collection.uuid,
                title: widget.collection.name,
              ),
            ),
          IconButton(
            tooltip: l10n.delete,
            icon: const Icon(Icons.delete_outline),
            onPressed: () async {
              await confirmDeleteCollection(context, ref, widget.collection);
              if (!context.mounted) return;
              if (ref.read(collectionRepositoryProvider).asData?.value
                      .collections
                      .any((c) => c.uuid == widget.collection.uuid) ==
                  false) {
                Navigator.of(context).pop();
              }
            },
          ),
        ],
      ),
      body: _loading
          ? const Center(child: CircularProgressIndicator())
          : RefreshIndicator(
              onRefresh: _loadItems,
              child: CustomScrollView(
                slivers: [
                  SliverPadding(
                    padding: const EdgeInsets.fromLTRB(16, 16, 16, 0),
                    sliver: SliverToBoxAdapter(
                      child: Row(
                        children: [
                          CircleAvatar(
                            backgroundColor: color.withAlpha(40),
                            radius: 28,
                            child: Icon(
                              namedIcon?.icon ?? Icons.folder_outlined,
                              color: color,
                              size: 28,
                            ),
                          ),
                          const SizedBox(width: 16),
                          Expanded(
                            child: Column(
                              crossAxisAlignment: CrossAxisAlignment.start,
                              children: [
                                if (widget.collection.description != null &&
                                    widget.collection.description!.isNotEmpty)
                                  Text(
                                    widget.collection.description!,
                                    style: Theme.of(context).textTheme.bodyMedium,
                                  ),
                                Text(
                                  l10n.memberCount(_refs.length),
                                  style: Theme.of(context)
                                      .textTheme
                                      .bodySmall
                                      ?.copyWith(
                                        color: colorScheme.onSurfaceVariant,
                                      ),
                                ),
                              ],
                            ),
                          ),
                        ],
                      ),
                    ),
                  ),
                  if (_refs.isEmpty)
                    SliverFillRemaining(
                      hasScrollBody: false,
                      child: Center(
                        child: Padding(
                          padding: const EdgeInsets.all(32),
                          child: Text(
                            l10n.noMembersInCollection,
                            textAlign: TextAlign.center,
                            style: Theme.of(context)
                                .textTheme
                                .bodyMedium
                                ?.copyWith(
                                  color: colorScheme.onSurfaceVariant,
                                ),
                          ),
                        ),
                      ),
                    )
                  else ...[
                    if (markerRefs.isNotEmpty) ...[
                      SliverPadding(
                        padding: const EdgeInsets.fromLTRB(16, 24, 16, 8),
                        sliver: SliverToBoxAdapter(
                          child: Text(
                            l10n.markersSection,
                            style: Theme.of(context).textTheme.titleSmall,
                          ),
                        ),
                      ),
                      SliverList.builder(
                        itemCount: markerRefs.length,
                        itemBuilder: (context, i) {
                          final ref = markerRefs[i];
                          final marker = markerByUuid[ref.uuid];
                          if (marker == null) {
                            return ListTile(
                              leading: const Icon(Icons.help_outline),
                              title: Text(ref.uuid),
                            );
                          }
                          return _MarkerRow(
                            marker: marker,
                            onChanged: _loadItems,
                          );
                        },
                      ),
                    ],
                    if (pathRefs.isNotEmpty) ...[
                      SliverPadding(
                        padding: const EdgeInsets.fromLTRB(16, 24, 16, 8),
                        sliver: SliverToBoxAdapter(
                          child: Text(
                            l10n.pathsSection,
                            style: Theme.of(context).textTheme.titleSmall,
                          ),
                        ),
                      ),
                      SliverList.builder(
                        itemCount: pathRefs.length,
                        itemBuilder: (context, i) {
                          final ref = pathRefs[i];
                          final path = pathByUuid[ref.uuid];
                          if (path == null) {
                            return ListTile(
                              leading: const Icon(Icons.help_outline),
                              title: Text(ref.uuid),
                            );
                          }
                          return _PathRow(path: path, onChanged: _loadItems);
                        },
                      ),
                    ],
                    const SliverPadding(
                      padding: EdgeInsets.only(bottom: 32),
                    ),
                  ],
                ],
              ),
            ),
    );
  }
}

class _MarkerRow extends ConsumerWidget {
  final Marker marker;
  final VoidCallback onChanged;

  const _MarkerRow({required this.marker, required this.onChanged});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final l10n = context.l10n;
    final colorScheme = Theme.of(context).colorScheme;
    final iconService = IconService();
    final namedIcon = iconService.getIcon(context, marker.icon);

    return ListTile(
      leading: CircleAvatar(
        backgroundColor: colorScheme.primaryContainer,
        child: Icon(namedIcon.icon, color: colorScheme.onPrimaryContainer),
      ),
      title: Text(marker.title, maxLines: 1, overflow: TextOverflow.ellipsis),
      subtitle: marker.description != null && marker.description!.isNotEmpty
          ? Text(marker.description!, maxLines: 1, overflow: TextOverflow.ellipsis)
          : null,
      trailing: const Icon(Icons.chevron_right),
      onTap: () async {
        final result = await showExclusiveSheet<MarkerInfoResult>(
          context,
          builder: (_) => MarkerInfoSheet(marker: marker),
        );
        if (!context.mounted) return;
        if (result == MarkerInfoResult.deleted) {
          AppSnackbars.success(context, l10n.markerDeleted);
          onChanged();
        } else if (result == MarkerInfoResult.updated) {
          AppSnackbars.success(context, l10n.markerUpdated);
          onChanged();
        }
      },
    );
  }
}

class _PathRow extends ConsumerWidget {
  final SavedPath path;
  final VoidCallback onChanged;

  const _PathRow({required this.path, required this.onChanged});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final l10n = context.l10n;
    final colorScheme = Theme.of(context).colorScheme;
    final pathColor = hexToColor(path.colorHex) ?? colorScheme.onSurfaceVariant;
    final iconService = IconService();
    final namedIcon = path.iconKey != null
        ? iconService.getIcon(context, path.iconKey)
        : null;

    return ListTile(
      leading: CircleAvatar(
        backgroundColor: pathColor.withAlpha(40),
        child: Icon(namedIcon?.icon ?? Icons.route, color: pathColor),
      ),
      title: Text(path.title, maxLines: 1, overflow: TextOverflow.ellipsis),
      subtitle: Text(
        '${(path.distance / 1000).toStringAsFixed(2)} km',
      ),
      trailing: const Icon(Icons.chevron_right),
      onTap: () async {
        final result = await showExclusiveSheet<PathDetailResult>(
          context,
          builder: (_) => PathInfoSheet(path: path),
        );
        if (!context.mounted) return;
        if (result == PathDetailResult.deleted) {
          AppSnackbars.success(context, l10n.pathDeleted);
          onChanged();
        } else if (result == PathDetailResult.updated) {
          AppSnackbars.success(context, l10n.pathUpdated);
          onChanged();
        }
      },
    );
  }
}
