import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:turbo/app/l10n/app_localizations.dart';
import 'package:turbo/app/tokens.dart';
import 'package:turbo/features/tile_providers/data/custom_provider_store.dart';
import 'package:turbo/features/tile_providers/models/custom_tile_provider.dart';
import 'package:turbo/features/tile_providers/models/tile_provider_config.dart';

/// Pushes the dedicated "Add custom map" page. Returns true if the user
/// saved a provider, false (or null) if they cancelled.
Future<bool?> pushAddCustomMapPage(BuildContext context) {
  return Navigator.of(context).push<bool>(
    MaterialPageRoute(builder: (_) => const AddCustomMapPage()),
  );
}

class AddCustomMapPage extends ConsumerStatefulWidget {
  const AddCustomMapPage({super.key});

  @override
  ConsumerState<AddCustomMapPage> createState() => _AddCustomMapPageState();
}

class _AddCustomMapPageState extends ConsumerState<AddCustomMapPage> {
  final _formKey = GlobalKey<FormState>();
  final _nameController = TextEditingController();
  final _urlController = TextEditingController();
  TileProviderCategory _category = TileProviderCategory.global;
  bool _saving = false;

  @override
  void dispose() {
    _nameController.dispose();
    _urlController.dispose();
    super.dispose();
  }

  Future<void> _submit() async {
    if (_saving) return;
    if (!_formKey.currentState!.validate()) return;
    setState(() => _saving = true);
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
    final textTheme = Theme.of(context).textTheme;
    final colorScheme = Theme.of(context).colorScheme;

    return Scaffold(
      appBar: AppBar(
        title: Text(l10n.addCustomMap),
        actions: [
          TextButton(
            onPressed: _saving ? null : _submit,
            child: Text(l10n.add),
          ),
          const SizedBox(width: AppSpacing.s),
        ],
      ),
      body: Form(
        key: _formKey,
        child: ListView(
          padding: const EdgeInsets.fromLTRB(
              AppSpacing.l, AppSpacing.l, AppSpacing.l, AppSpacing.xl),
          children: [
            // --- Name ---
            Text(
              l10n.customMapNameHint,
              style: textTheme.labelLarge,
            ),
            const SizedBox(height: AppSpacing.s),
            TextFormField(
              controller: _nameController,
              autofocus: true,
              decoration: const InputDecoration(
                border: OutlineInputBorder(),
              ),
              validator: (v) {
                if (v == null || v.trim().isEmpty) {
                  return l10n.customMapNameRequired;
                }
                return null;
              },
            ),
            const SizedBox(height: AppSpacing.xl),

            // --- URL template ---
            Text(
              l10n.customMapUrlHint,
              style: textTheme.labelLarge,
            ),
            const SizedBox(height: AppSpacing.s),
            TextFormField(
              controller: _urlController,
              keyboardType: TextInputType.url,
              autocorrect: false,
              decoration: InputDecoration(
                border: const OutlineInputBorder(),
                hintText: 'https://example.com/tiles/{z}/{x}/{y}.png',
                hintStyle: textTheme.bodyMedium?.copyWith(
                  color: colorScheme.onSurfaceVariant
                      .withValues(alpha: 0.6),
                ),
              ),
              minLines: 1,
              maxLines: 3,
              validator: (v) {
                final err = CustomTileProvider
                    .validateUrlTemplate(v ?? '');
                if (err != null) return l10n.customMapInvalidUrl;
                return null;
              },
            ),
            const SizedBox(height: AppSpacing.s),
            Text(
              l10n.customMapUrlHelp,
              style: textTheme.bodySmall?.copyWith(
                color: colorScheme.onSurfaceVariant,
              ),
            ),
            const SizedBox(height: AppSpacing.xl),

            // --- Category ---
            Text(
              l10n.customMapCategoryLabel,
              style: textTheme.labelLarge,
            ),
            const SizedBox(height: AppSpacing.s),
            SegmentedButton<TileProviderCategory>(
              segments: [
                ButtonSegment(
                  value: TileProviderCategory.global,
                  label: Text(l10n.customMapCategoryGlobal),
                ),
                ButtonSegment(
                  value: TileProviderCategory.local,
                  label: Text(l10n.customMapCategoryLocal),
                ),
                ButtonSegment(
                  value: TileProviderCategory.overlay,
                  label: Text(l10n.customMapCategoryOverlay),
                ),
              ],
              selected: {_category},
              onSelectionChanged: (next) =>
                  setState(() => _category = next.first),
            ),
          ],
        ),
      ),
    );
  }
}
