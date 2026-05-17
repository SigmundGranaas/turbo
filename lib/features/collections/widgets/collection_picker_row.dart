import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:turbo/app/l10n/app_localizations.dart';
import 'package:turbo/features/saved_paths/api.dart' show hexToColor;

import '../data/collection_repository.dart';
import 'add_to_collection_sheet.dart';

/// Tappable row showing the collections an as-yet-uncreated item will be
/// added to. Shows a single "+ Add to collection" chip when empty, otherwise
/// the selected collections as chips followed by an inline `+` button.
///
/// Used by `CreateLocationSheet` and `SavePathSheet` to attach the new
/// marker / path to one or more collections at the moment it is saved.
class CollectionPickerRow extends ConsumerWidget {
  final Set<String> selectedUuids;
  final ValueChanged<Set<String>> onChanged;

  const CollectionPickerRow({
    super.key,
    required this.selectedUuids,
    required this.onChanged,
  });

  Future<void> _open(BuildContext context) async {
    final picked = await AddToCollectionSheet.pick(
      context,
      initialSelected: selectedUuids,
    );
    if (picked != null) onChanged(picked);
  }

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final l10n = context.l10n;
    final colorScheme = Theme.of(context).colorScheme;
    final asyncState = ref.watch(collectionRepositoryProvider);

    return asyncState.maybeWhen(
      data: (state) {
        final byUuid = {for (final c in state.collections) c.uuid: c};
        final present = selectedUuids.where(byUuid.containsKey).toList();

        return SizedBox(
          height: 36,
          child: SingleChildScrollView(
            scrollDirection: Axis.horizontal,
            child: Row(
              children: [
                if (present.isEmpty)
                  ActionChip(
                    avatar: Icon(
                      Icons.folder_outlined,
                      size: 16,
                      color: colorScheme.primary,
                    ),
                    label: Text(l10n.addToCollection),
                    onPressed: () => _open(context),
                    visualDensity: VisualDensity.compact,
                  )
                else ...[
                  for (final id in present)
                    Padding(
                      padding: const EdgeInsets.only(right: 6),
                      child: ActionChip(
                        avatar: Icon(
                          Icons.folder_outlined,
                          size: 16,
                          color: hexToColor(byUuid[id]!.colorHex) ??
                              colorScheme.primary,
                        ),
                        label: Text(byUuid[id]!.name),
                        onPressed: () => _open(context),
                        visualDensity: VisualDensity.compact,
                      ),
                    ),
                  IconButton(
                    tooltip: l10n.addToCollection,
                    iconSize: 20,
                    visualDensity: VisualDensity.compact,
                    icon: const Icon(Icons.add_circle_outline),
                    onPressed: () => _open(context),
                  ),
                ],
              ],
            ),
          ),
        );
      },
      orElse: () => const SizedBox.shrink(),
    );
  }
}
