import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:turbo/app/l10n/app_localizations.dart';
import 'package:turbo/core/widgets/app_snackbars.dart';
import 'package:turbo/features/markers/api.dart';
import 'package:turbo/features/saved_paths/api.dart' show hexToColor;

import '../data/collection_repository.dart';
import '../models/collection.dart';
import '../models/collection_item_ref.dart';
import 'create_or_edit_collection_sheet.dart';

class AddToCollectionSheet extends ConsumerStatefulWidget {
  final CollectionItemRef itemRef;

  const AddToCollectionSheet({super.key, required this.itemRef});

  static Future<void> show(BuildContext context, CollectionItemRef ref) {
    return showModalBottomSheet<void>(
      context: context,
      isScrollControlled: true,
      useSafeArea: true,
      shape: const RoundedRectangleBorder(
        borderRadius: BorderRadius.vertical(top: Radius.circular(16)),
      ),
      builder: (_) => AddToCollectionSheet(itemRef: ref),
    );
  }

  @override
  ConsumerState<AddToCollectionSheet> createState() =>
      _AddToCollectionSheetState();
}

class _AddToCollectionSheetState extends ConsumerState<AddToCollectionSheet> {
  late Set<String> _selected;
  bool _initialized = false;
  bool _saving = false;

  void _initSelection(CollectionRepositoryState state) {
    if (_initialized) return;
    _selected = state.collectionsFor(widget.itemRef).toSet();
    _initialized = true;
  }

  Future<void> _createNew() async {
    final created = await CreateOrEditCollectionSheet.show(context);
    if (created != null && mounted) {
      setState(() => _selected.add(created.uuid));
    }
  }

  Future<void> _save() async {
    final l10n = context.l10n;
    setState(() => _saving = true);
    try {
      await ref
          .read(collectionRepositoryProvider.notifier)
          .setMembership(widget.itemRef, _selected);
      if (!mounted) return;
      Navigator.of(context).pop();
    } catch (e) {
      if (mounted) {
        AppSnackbars.error(context, l10n.errorSavingCollection(e.toString()));
      }
    } finally {
      if (mounted) setState(() => _saving = false);
    }
  }

  @override
  Widget build(BuildContext context) {
    final l10n = context.l10n;
    final textTheme = Theme.of(context).textTheme;
    final colorScheme = Theme.of(context).colorScheme;
    final mediaQuery = MediaQuery.of(context);
    final bottomPadding = mediaQuery.viewInsets.bottom > 0
        ? mediaQuery.viewInsets.bottom
        : mediaQuery.padding.bottom;
    final asyncState = ref.watch(collectionRepositoryProvider);

    return Padding(
      padding: EdgeInsets.fromLTRB(16, 16, 16, 16 + bottomPadding),
      child: asyncState.when(
        loading: () => const Padding(
          padding: EdgeInsets.all(24),
          child: Center(child: CircularProgressIndicator()),
        ),
        error: (e, _) => Padding(
          padding: const EdgeInsets.all(24),
          child: Text('$e'),
        ),
        data: (state) {
          _initSelection(state);
          final collections = state.collections;
          return Column(
            mainAxisSize: MainAxisSize.min,
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              Row(
                children: [
                  Expanded(
                    child: Text(
                      l10n.addToCollection,
                      style: textTheme.titleLarge,
                    ),
                  ),
                  IconButton(
                    onPressed: () => Navigator.pop(context),
                    icon: const Icon(Icons.close),
                  ),
                ],
              ),
              const SizedBox(height: 8),
              ListTile(
                contentPadding: EdgeInsets.zero,
                leading: CircleAvatar(
                  backgroundColor: colorScheme.primaryContainer,
                  child: Icon(Icons.add, color: colorScheme.onPrimaryContainer),
                ),
                title: Text(l10n.newCollection),
                onTap: _saving ? null : _createNew,
              ),
              if (collections.isEmpty)
                Padding(
                  padding: const EdgeInsets.symmetric(vertical: 24),
                  child: Text(
                    l10n.noCollectionsYet,
                    textAlign: TextAlign.center,
                    style: textTheme.bodyMedium?.copyWith(
                      color: colorScheme.onSurfaceVariant,
                    ),
                  ),
                )
              else
                Flexible(
                  child: ListView.builder(
                    shrinkWrap: true,
                    itemCount: collections.length,
                    itemBuilder: (context, i) {
                      final c = collections[i];
                      return _CollectionCheckRow(
                        collection: c,
                        selected: _selected.contains(c.uuid),
                        onChanged: (v) {
                          setState(() {
                            if (v == true) {
                              _selected.add(c.uuid);
                            } else {
                              _selected.remove(c.uuid);
                            }
                          });
                        },
                      );
                    },
                  ),
                ),
              const SizedBox(height: 12),
              FilledButton(
                onPressed: _saving ? null : _save,
                child: Text(l10n.save),
              ),
            ],
          );
        },
      ),
    );
  }
}

class _CollectionCheckRow extends StatelessWidget {
  final Collection collection;
  final bool selected;
  final ValueChanged<bool?> onChanged;

  const _CollectionCheckRow({
    required this.collection,
    required this.selected,
    required this.onChanged,
  });

  @override
  Widget build(BuildContext context) {
    final colorScheme = Theme.of(context).colorScheme;
    final color = hexToColor(collection.colorHex) ?? colorScheme.primary;
    final iconService = IconService();
    final namedIcon = collection.iconKey != null
        ? iconService.getIcon(context, collection.iconKey)
        : null;

    return CheckboxListTile(
      contentPadding: EdgeInsets.zero,
      value: selected,
      onChanged: onChanged,
      title: Text(collection.name),
      subtitle: collection.description != null && collection.description!.isNotEmpty
          ? Text(collection.description!)
          : null,
      secondary: CircleAvatar(
        backgroundColor: color.withAlpha(40),
        child: Icon(
          namedIcon?.icon ?? Icons.folder_outlined,
          color: color,
        ),
      ),
    );
  }
}
