import 'package:flutter/material.dart';
import 'package:turbo/core/widgets/exclusive_sheet.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:turbo/app/l10n/app_localizations.dart';
import 'package:turbo/core/widgets/app_snackbars.dart';
import 'package:turbo/features/markers/api.dart';
import 'package:turbo/features/saved_paths/api.dart' show hexToColor;

import '../data/collection_repository.dart';
import '../models/collection.dart';
import '../models/collection_item_ref.dart';
import 'create_or_edit_collection_sheet.dart';

/// Sheet for picking collections.
///
/// Two modes:
/// - Binding mode (`itemRef != null`): writes membership directly to the
///   repository on save and pops with no result.
/// - Picker mode (`itemRef == null`): seeds the checkboxes from
///   [initialSelected] and pops with the chosen `Set<String>`. Used by the
///   create sheets where the item does not yet have a UUID.
class AddToCollectionSheet extends ConsumerStatefulWidget {
  final CollectionItemRef? itemRef;
  final Set<String> initialSelected;

  const AddToCollectionSheet({
    super.key,
    this.itemRef,
    this.initialSelected = const {},
  });

  /// Binding mode — writes membership for [ref] when the user saves.
  static Future<void> show(BuildContext context, CollectionItemRef ref) {
    return showExclusiveSheet<void>(
      context,
      replace: false,
      shape: const RoundedRectangleBorder(
        borderRadius: BorderRadius.vertical(top: Radius.circular(16)),
      ),
      builder: (_) => AddToCollectionSheet(itemRef: ref),
    );
  }

  /// Picker mode — returns the selected collection UUIDs, or null if the
  /// user dismissed the sheet without saving.
  static Future<Set<String>?> pick(
    BuildContext context, {
    Set<String> initialSelected = const {},
  }) {
    return showExclusiveSheet<Set<String>>(
      context,
      replace: false,
      shape: const RoundedRectangleBorder(
        borderRadius: BorderRadius.vertical(top: Radius.circular(16)),
      ),
      builder: (_) => AddToCollectionSheet(initialSelected: initialSelected),
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
    final itemRef = widget.itemRef;
    _selected = itemRef != null
        ? state.collectionsFor(itemRef).toSet()
        : widget.initialSelected.toSet();
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
    final itemRef = widget.itemRef;
    if (itemRef == null) {
      Navigator.of(context).pop(_selected);
      return;
    }
    setState(() => _saving = true);
    try {
      await ref
          .read(collectionRepositoryProvider.notifier)
          .setMembership(itemRef, _selected);
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
                      return _CollectionSwitchRow(
                        collection: c,
                        selected: _selected.contains(c.uuid),
                        onChanged: (v) {
                          setState(() {
                            if (v) {
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

class _CollectionSwitchRow extends StatelessWidget {
  final Collection collection;
  final bool selected;
  final ValueChanged<bool> onChanged;

  const _CollectionSwitchRow({
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

    return SwitchListTile(
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
