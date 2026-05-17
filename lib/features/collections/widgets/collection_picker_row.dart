import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:turbo/app/l10n/app_localizations.dart';
import 'package:turbo/features/saved_paths/api.dart' show hexToColor;

import '../data/collection_repository.dart';
import 'add_to_collection_sheet.dart';

/// Tappable surface row letting the user attach the marker / path being
/// created to one or more collections. Matches the surface-card pattern used
/// by `PathCustomizationControls` so it sits naturally next to the Icon row.
///
/// Empty state shows the leading folder avatar with an "Add to collection"
/// label and a chevron. With selections it shows the picked collections as
/// chips inside the same surface, still tap-to-edit.
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
    final textTheme = Theme.of(context).textTheme;
    final asyncState = ref.watch(collectionRepositoryProvider);

    return asyncState.maybeWhen(
      data: (state) {
        final byUuid = {for (final c in state.collections) c.uuid: c};
        final present = selectedUuids.where(byUuid.containsKey).toList();
        final hasSelection = present.isNotEmpty;

        return Material(
          color: colorScheme.surfaceContainer,
          shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(16)),
          clipBehavior: Clip.antiAlias,
          child: InkWell(
            onTap: () => _open(context),
            child: Padding(
              padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 12),
              child: Row(
                children: [
                  Container(
                    width: 40,
                    height: 40,
                    decoration: BoxDecoration(
                      color: hasSelection
                          ? colorScheme.primaryContainer
                          : colorScheme.surfaceContainerHighest,
                      shape: BoxShape.circle,
                    ),
                    child: Icon(
                      Icons.folder_outlined,
                      color: hasSelection
                          ? colorScheme.onPrimaryContainer
                          : colorScheme.onSurfaceVariant,
                      size: 24,
                    ),
                  ),
                  const SizedBox(width: 16),
                  Expanded(
                    child: hasSelection
                        ? Wrap(
                            spacing: 6,
                            runSpacing: 4,
                            children: [
                              for (final id in present)
                                Chip(
                                  avatar: Icon(
                                    Icons.folder_outlined,
                                    size: 14,
                                    color: hexToColor(byUuid[id]!.colorHex) ??
                                        colorScheme.primary,
                                  ),
                                  label: Text(byUuid[id]!.name),
                                  visualDensity: VisualDensity.compact,
                                  materialTapTargetSize:
                                      MaterialTapTargetSize.shrinkWrap,
                                ),
                            ],
                          )
                        : Text(
                            l10n.addToCollection,
                            style: textTheme.bodyLarge?.copyWith(
                              color: colorScheme.onSurface,
                            ),
                          ),
                  ),
                  Icon(
                    Icons.chevron_right,
                    color: colorScheme.onSurfaceVariant,
                  ),
                ],
              ),
            ),
          ),
        );
      },
      orElse: () => const SizedBox.shrink(),
    );
  }
}
