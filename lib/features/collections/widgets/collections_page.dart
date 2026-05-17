import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:turbo/app/l10n/app_localizations.dart';
import 'package:turbo/core/widgets/app_snackbars.dart';
import 'package:turbo/features/markers/api.dart';
import 'package:turbo/features/saved_paths/api.dart' show hexToColor;

import '../data/collection_repository.dart';
import '../data/collection_visibility_provider.dart';
import '../models/collection.dart';
import 'collection_detail_page.dart';
import 'create_or_edit_collection_sheet.dart';

class CollectionsPage extends ConsumerWidget {
  const CollectionsPage({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final l10n = context.l10n;
    final asyncState = ref.watch(collectionRepositoryProvider);

    return Scaffold(
      appBar: AppBar(title: Text(l10n.collections)),
      floatingActionButton: FloatingActionButton.extended(
        onPressed: () => CreateOrEditCollectionSheet.show(context),
        icon: const Icon(Icons.add),
        label: Text(l10n.newCollection),
      ),
      body: asyncState.when(
        loading: () => const Center(child: CircularProgressIndicator()),
        error: (e, _) => Center(child: Text('$e')),
        data: (state) {
          if (state.collections.isEmpty) {
            return _EmptyState();
          }
          return ListView.separated(
            padding: const EdgeInsets.fromLTRB(8, 8, 8, 96),
            itemCount: state.collections.length,
            separatorBuilder: (_, _) => const SizedBox(height: 4),
            itemBuilder: (context, i) {
              final c = state.collections[i];
              return _CollectionRow(
                collection: c,
                memberCount: state.memberCountFor(c.uuid),
              );
            },
          );
        },
      ),
    );
  }
}

class _EmptyState extends StatelessWidget {
  @override
  Widget build(BuildContext context) {
    final l10n = context.l10n;
    final textTheme = Theme.of(context).textTheme;
    final colorScheme = Theme.of(context).colorScheme;
    return Padding(
      padding: const EdgeInsets.all(32),
      child: Column(
        mainAxisAlignment: MainAxisAlignment.center,
        children: [
          Icon(
            Icons.folder_open_outlined,
            size: 64,
            color: colorScheme.onSurfaceVariant,
          ),
          const SizedBox(height: 16),
          Text(
            l10n.noCollectionsYet,
            style: textTheme.titleMedium,
            textAlign: TextAlign.center,
          ),
          const SizedBox(height: 8),
          Text(
            l10n.noCollectionsHint,
            style: textTheme.bodyMedium?.copyWith(
              color: colorScheme.onSurfaceVariant,
            ),
            textAlign: TextAlign.center,
          ),
        ],
      ),
    );
  }
}

class _CollectionRow extends ConsumerWidget {
  final Collection collection;
  final int memberCount;

  const _CollectionRow({required this.collection, required this.memberCount});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final l10n = context.l10n;
    final colorScheme = Theme.of(context).colorScheme;
    final color = hexToColor(collection.colorHex) ?? colorScheme.primary;
    final iconService = IconService();
    final namedIcon = collection.iconKey != null
        ? iconService.getIcon(context, collection.iconKey)
        : null;
    final visibility = ref.watch(collectionVisibilityProvider);
    final isVisible = visibility[collection.uuid] ?? true;

    return Card(
      margin: EdgeInsets.zero,
      child: ListTile(
        leading: CircleAvatar(
          backgroundColor: color.withAlpha(40),
          child: Icon(
            namedIcon?.icon ?? Icons.folder_outlined,
            color: color,
          ),
        ),
        title: Text(collection.name),
        subtitle: Text(l10n.memberCount(memberCount)),
        trailing: IconButton(
          tooltip: isVisible ? l10n.visibleOnMap : l10n.hiddenOnMap,
          icon: Icon(
            isVisible ? Icons.visibility_outlined : Icons.visibility_off_outlined,
            color: isVisible
                ? colorScheme.onSurface
                : colorScheme.onSurfaceVariant,
          ),
          onPressed: () => ref
              .read(collectionVisibilityProvider.notifier)
              .toggle(collection.uuid),
        ),
        onTap: () {
          Navigator.of(context).push(
            MaterialPageRoute(
              builder: (_) => CollectionDetailPage(collectionUuid: collection.uuid),
            ),
          );
        },
      ),
    );
  }
}

/// Helper for showing a delete confirmation from any consumer.
Future<void> confirmDeleteCollection(
  BuildContext context,
  WidgetRef ref,
  Collection collection,
) async {
  final l10n = context.l10n;
  final confirmed = await showDialog<bool>(
    context: context,
    builder: (dialogContext) => AlertDialog.adaptive(
      title: Text(l10n.confirmDeleteCollectionTitle),
      content: Text(l10n.confirmDeleteCollectionMessage),
      actions: [
        TextButton(
          onPressed: () => Navigator.of(dialogContext).pop(false),
          child: Text(l10n.cancel),
        ),
        FilledButton(
          style: FilledButton.styleFrom(
            backgroundColor: Theme.of(context).colorScheme.error,
            foregroundColor: Theme.of(context).colorScheme.onError,
          ),
          onPressed: () => Navigator.of(dialogContext).pop(true),
          child: Text(l10n.delete),
        ),
      ],
    ),
  );
  if (confirmed != true) return;
  try {
    await ref
        .read(collectionRepositoryProvider.notifier)
        .deleteCollection(collection.uuid);
    if (context.mounted) {
      AppSnackbars.success(context, l10n.collectionDeleted);
    }
  } catch (e) {
    if (context.mounted) {
      AppSnackbars.error(
        context,
        l10n.errorDeletingCollection(e.toString()),
      );
    }
  }
}
