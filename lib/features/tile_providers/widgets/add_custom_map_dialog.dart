import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:turbo/app/l10n/app_localizations.dart';
import 'package:turbo/features/tile_providers/data/custom_provider_store.dart';
import 'package:turbo/features/tile_providers/models/custom_tile_provider.dart';
import 'package:turbo/features/tile_providers/models/tile_provider_config.dart';

/// Shows the "Add custom map" dialog. Returns true if the user saved a
/// provider, false if they cancelled.
Future<bool> showAddCustomMapDialog(BuildContext context) async {
  final result = await showDialog<bool>(
    context: context,
    builder: (_) => const _AddCustomMapDialog(),
  );
  return result ?? false;
}

class _AddCustomMapDialog extends ConsumerStatefulWidget {
  const _AddCustomMapDialog();

  @override
  ConsumerState<_AddCustomMapDialog> createState() =>
      _AddCustomMapDialogState();
}

class _AddCustomMapDialogState extends ConsumerState<_AddCustomMapDialog> {
  final _formKey = GlobalKey<FormState>();
  final _nameController = TextEditingController();
  final _urlController = TextEditingController();
  TileProviderCategory _category = TileProviderCategory.global;

  @override
  void dispose() {
    _nameController.dispose();
    _urlController.dispose();
    super.dispose();
  }

  Future<void> _submit() async {
    if (!_formKey.currentState!.validate()) return;
    final provider = CustomTileProvider(
      id: 'custom_${DateTime.now().microsecondsSinceEpoch}',
      displayName: _nameController.text.trim(),
      urlTemplate: _urlController.text.trim(),
      category: _category,
    );
    await ref.read(customProviderStoreProvider.notifier).add(provider);
    if (!mounted) return;
    Navigator.of(context).pop(true);
  }

  @override
  Widget build(BuildContext context) {
    final l10n = context.l10n;
    return AlertDialog.adaptive(
      title: Text(l10n.addCustomMap),
      content: SingleChildScrollView(
        child: Form(
          key: _formKey,
          child: Column(
            mainAxisSize: MainAxisSize.min,
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              TextFormField(
                controller: _nameController,
                autofocus: true,
                decoration: InputDecoration(
                  labelText: l10n.customMapNameHint,
                  border: const OutlineInputBorder(),
                ),
                validator: (v) {
                  if (v == null || v.trim().isEmpty) {
                    return l10n.customMapNameRequired;
                  }
                  return null;
                },
              ),
              const SizedBox(height: 16),
              TextFormField(
                controller: _urlController,
                decoration: InputDecoration(
                  labelText: l10n.customMapUrlHint,
                  helperText: l10n.customMapUrlHelp,
                  helperMaxLines: 2,
                  border: const OutlineInputBorder(),
                ),
                keyboardType: TextInputType.url,
                validator: (v) {
                  final err = CustomTileProvider
                      .validateUrlTemplate(v ?? '');
                  if (err != null) return l10n.customMapInvalidUrl;
                  return null;
                },
              ),
              const SizedBox(height: 16),
              DropdownButtonFormField<TileProviderCategory>(
                initialValue: _category,
                decoration: InputDecoration(
                  labelText: l10n.customMapCategoryLabel,
                  border: const OutlineInputBorder(),
                ),
                items: [
                  DropdownMenuItem(
                    value: TileProviderCategory.global,
                    child: Text(l10n.customMapCategoryGlobal),
                  ),
                  DropdownMenuItem(
                    value: TileProviderCategory.local,
                    child: Text(l10n.customMapCategoryLocal),
                  ),
                  DropdownMenuItem(
                    value: TileProviderCategory.overlay,
                    child: Text(l10n.customMapCategoryOverlay),
                  ),
                ],
                onChanged: (v) {
                  if (v != null) setState(() => _category = v);
                },
              ),
            ],
          ),
        ),
      ),
      actions: [
        TextButton(
          onPressed: () => Navigator.of(context).pop(false),
          child: Text(l10n.cancel),
        ),
        FilledButton(
          onPressed: _submit,
          child: Text(l10n.add),
        ),
      ],
    );
  }
}
