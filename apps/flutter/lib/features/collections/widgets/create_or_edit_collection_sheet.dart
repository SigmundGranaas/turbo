import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:turbo/app/l10n/app_localizations.dart';
import 'package:turbo/core/widgets/app_snackbars.dart';
import 'package:turbo/core/widgets/color_circle.dart';
import 'package:turbo/features/markers/api.dart';
import 'package:turbo/features/saved_paths/api.dart' show colorToHex, hexToColor, pathColorPalette;

import '../data/collection_repository.dart';
import '../models/collection.dart';

class CreateOrEditCollectionSheet extends ConsumerStatefulWidget {
  final Collection? existing;

  const CreateOrEditCollectionSheet({super.key, this.existing});

  static Future<Collection?> show(
    BuildContext context, {
    Collection? existing,
  }) {
    return showModalBottomSheet<Collection>(
      context: context,
      isScrollControlled: true,
      useSafeArea: true,
      shape: const RoundedRectangleBorder(
        borderRadius: BorderRadius.vertical(top: Radius.circular(16)),
      ),
      builder: (_) => CreateOrEditCollectionSheet(existing: existing),
    );
  }

  @override
  ConsumerState<CreateOrEditCollectionSheet> createState() =>
      _CreateOrEditCollectionSheetState();
}

class _CreateOrEditCollectionSheetState
    extends ConsumerState<CreateOrEditCollectionSheet> {
  final _formKey = GlobalKey<FormState>();
  late final TextEditingController _nameController;
  late final TextEditingController _descController;
  String? _colorHex;
  String? _iconKey;
  bool _saving = false;

  @override
  void initState() {
    super.initState();
    final existing = widget.existing;
    _nameController = TextEditingController(text: existing?.name ?? '');
    _descController = TextEditingController(text: existing?.description ?? '');
    _colorHex = existing?.colorHex;
    _iconKey = existing?.iconKey;
  }

  @override
  void dispose() {
    _nameController.dispose();
    _descController.dispose();
    super.dispose();
  }

  Future<void> _save() async {
    if (!_formKey.currentState!.validate()) return;
    final l10n = context.l10n;
    setState(() => _saving = true);
    try {
      final repo = ref.read(collectionRepositoryProvider.notifier);
      final existing = widget.existing;
      Collection saved;
      if (existing == null) {
        saved = Collection(
          name: _nameController.text.trim(),
          description: _descController.text.trim().isEmpty
              ? null
              : _descController.text.trim(),
          colorHex: _colorHex,
          iconKey: _iconKey,
        );
        await repo.create(saved);
      } else {
        saved = existing.copyWith(
          name: _nameController.text.trim(),
          description: _descController.text.trim().isEmpty
              ? null
              : _descController.text.trim(),
          clearDescription: _descController.text.trim().isEmpty,
          colorHex: _colorHex,
          clearColorHex: _colorHex == null,
          iconKey: _iconKey,
          clearIconKey: _iconKey == null,
        );
        await repo.updateCollection(saved);
      }
      if (!mounted) return;
      Navigator.of(context).pop(saved);
    } catch (error) {
      if (!mounted) return;
      AppSnackbars.error(
        context,
        l10n.errorSavingCollection(error.toString()),
      );
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
    final isEdit = widget.existing != null;
    final selectedColor = hexToColor(_colorHex);

    return Padding(
      padding: EdgeInsets.fromLTRB(16, 16, 16, 16 + bottomPadding),
      child: Form(
        key: _formKey,
        child: SingleChildScrollView(
          child: Column(
            mainAxisSize: MainAxisSize.min,
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              Row(
                children: [
                  Expanded(
                    child: Text(
                      isEdit ? l10n.editCollection : l10n.newCollection,
                      style: textTheme.titleLarge,
                    ),
                  ),
                  IconButton(
                    onPressed: () => Navigator.pop(context),
                    icon: const Icon(Icons.close),
                  ),
                ],
              ),
              const SizedBox(height: 16),
              TextFormField(
                controller: _nameController,
                decoration: InputDecoration(
                  labelText: l10n.collectionName,
                  border: const OutlineInputBorder(),
                ),
                validator: (value) {
                  if (value == null || value.trim().isEmpty) {
                    return l10n.pleaseEnterName;
                  }
                  return null;
                },
              ),
              const SizedBox(height: 12),
              TextFormField(
                controller: _descController,
                decoration: InputDecoration(
                  labelText: l10n.descriptionOptional,
                  border: const OutlineInputBorder(),
                ),
                maxLines: 2,
              ),
              const SizedBox(height: 16),
              Text(l10n.color, style: textTheme.titleSmall),
              const SizedBox(height: 8),
              SingleChildScrollView(
                scrollDirection: Axis.horizontal,
                child: Row(
                  children: [
                    ColorCircle(
                      color: null,
                      isSelected: selectedColor == null,
                      onTap: () => setState(() => _colorHex = null),
                      label: l10n.defaultColor,
                      colorScheme: colorScheme,
                    ),
                    ...pathColorPalette.map((c) => ColorCircle(
                          color: c,
                          isSelected: selectedColor != null &&
                              colorToHex(selectedColor) == colorToHex(c),
                          onTap: () =>
                              setState(() => _colorHex = colorToHex(c)),
                          colorScheme: colorScheme,
                        )),
                  ],
                ),
              ),
              const SizedBox(height: 16),
              _IconPickerRow(
                iconKey: _iconKey,
                onChanged: (key) => setState(() => _iconKey = key),
              ),
              const SizedBox(height: 24),
              FilledButton(
                onPressed: _saving ? null : _save,
                child: Text(isEdit ? l10n.saveChanges : l10n.save),
              ),
            ],
          ),
        ),
      ),
    );
  }
}

class _IconPickerRow extends StatelessWidget {
  final String? iconKey;
  final ValueChanged<String?> onChanged;

  const _IconPickerRow({required this.iconKey, required this.onChanged});

  @override
  Widget build(BuildContext context) {
    final l10n = context.l10n;
    final colorScheme = Theme.of(context).colorScheme;
    final textTheme = Theme.of(context).textTheme;
    final iconService = IconService();
    final hasIcon = iconKey != null;
    final namedIcon = hasIcon ? iconService.getIcon(context, iconKey) : null;

    return Material(
      color: colorScheme.surfaceContainer,
      shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(16)),
      clipBehavior: Clip.antiAlias,
      child: InkWell(
        onTap: () async {
          final result = await IconSelectionPage.show(context);
          FocusManager.instance.primaryFocus?.unfocus();
          if (result != null) {
            onChanged(result.title);
          }
        },
        child: Padding(
          padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 12),
          child: Row(
            children: [
              Container(
                width: 40,
                height: 40,
                decoration: BoxDecoration(
                  color: hasIcon
                      ? colorScheme.primaryContainer
                      : colorScheme.surfaceContainerHighest,
                  shape: BoxShape.circle,
                ),
                child: Icon(
                  hasIcon ? namedIcon!.icon : Icons.add,
                  color: hasIcon
                      ? colorScheme.onPrimaryContainer
                      : colorScheme.onSurfaceVariant,
                ),
              ),
              const SizedBox(width: 16),
              Expanded(
                child: Text(
                  hasIcon
                      ? (namedIcon!.localizedTitle ?? namedIcon.title)
                      : l10n.icon,
                  style: textTheme.bodyLarge,
                ),
              ),
              if (hasIcon)
                IconButton(
                  icon: const Icon(Icons.clear),
                  tooltip: l10n.removeIcon,
                  onPressed: () => onChanged(null),
                )
              else
                Icon(Icons.chevron_right, color: colorScheme.onSurfaceVariant),
            ],
          ),
        ),
      ),
    );
  }
}
