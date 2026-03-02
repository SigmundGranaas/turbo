import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:turbo/l10n/app_localizations.dart';
import 'package:turbo/core/widgets/buttons/primary_button.dart';
import 'package:turbo/core/widgets/buttons/secondary_button.dart';
import '../models/saved_path.dart';
import '../models/path_style.dart';
import '../data/saved_path_repository.dart';
import 'export_options_sheet.dart';
import 'path_customization_controls.dart';

enum PathDetailResult { updated, deleted }

class PathDetailSheet extends ConsumerStatefulWidget {
  final SavedPath path;

  const PathDetailSheet({super.key, required this.path});

  @override
  ConsumerState<PathDetailSheet> createState() => _PathDetailSheetState();
}

class _PathDetailSheetState extends ConsumerState<PathDetailSheet> {
  final _formKey = GlobalKey<FormState>();
  late final TextEditingController _nameController;
  late final TextEditingController _descriptionController;
  bool _isLoading = false;
  bool _isDeleting = false;
  late bool _showDescription;
  late Color? _selectedColor;
  late String? _selectedIconKey;
  late bool _isSmoothing;
  late PathLineStyle _lineStyle;

  @override
  void initState() {
    super.initState();
    _nameController = TextEditingController(text: widget.path.title);
    _descriptionController = TextEditingController(text: widget.path.description);
    _showDescription = widget.path.description != null && widget.path.description!.isNotEmpty;
    _selectedColor = hexToColor(widget.path.colorHex);
    _selectedIconKey = widget.path.iconKey;
    _isSmoothing = widget.path.smoothing;
    _lineStyle = PathLineStyle.fromKey(widget.path.lineStyleKey);
  }

  @override
  void dispose() {
    _nameController.dispose();
    _descriptionController.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final l10n = context.l10n;
    final textTheme = Theme.of(context).textTheme;
    final mediaQuery = MediaQuery.of(context);
    final bottomPadding = mediaQuery.viewInsets.bottom > 0
        ? mediaQuery.viewInsets.bottom
        : mediaQuery.padding.bottom;

    return Padding(
      padding: EdgeInsets.fromLTRB(16, 16, 16, 16 + bottomPadding),
      child: Form(
        key: _formKey,
        child: Column(
          mainAxisSize: MainAxisSize.min,
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            Row(
              mainAxisAlignment: MainAxisAlignment.spaceBetween,
              children: [
                Text(l10n.editPath, style: textTheme.titleLarge),
                IconButton(
                  onPressed: () => Navigator.pop(context),
                  icon: const Icon(Icons.close),
                ),
              ],
            ),
            const SizedBox(height: 24),
            TextFormField(
              controller: _nameController,
              decoration: InputDecoration(
                labelText: l10n.pathName,
                border: const OutlineInputBorder(),
              ),
              validator: (value) {
                if (value == null || value.trim().isEmpty) {
                  return l10n.pleaseEnterName;
                }
                return null;
              },
            ),
            const SizedBox(height: 16),
            if (_showDescription)
              TextFormField(
                controller: _descriptionController,
                decoration: InputDecoration(
                  labelText: l10n.descriptionOptional,
                  border: const OutlineInputBorder(),
                ),
                maxLines: 2,
              )
            else
              Align(
                alignment: Alignment.centerLeft,
                child: TextButton.icon(
                  onPressed: () => setState(() => _showDescription = true),
                  icon: const Icon(Icons.add, size: 18),
                  label: Text(l10n.addDescription),
                ),
              ),
            const SizedBox(height: 16),
            PathCustomizationControls(
              selectedColor: _selectedColor,
              onColorChanged: (c) => setState(() => _selectedColor = c),
              selectedIconKey: _selectedIconKey,
              onIconChanged: (k) => setState(() => _selectedIconKey = k),
              isSmoothing: _isSmoothing,
              onSmoothingChanged: (v) => setState(() => _isSmoothing = v),
              lineStyle: _lineStyle,
              onLineStyleChanged: (s) => setState(() => _lineStyle = s),
              initiallyExpanded: true,
            ),
            const SizedBox(height: 16),
            Text(
              '${l10n.totalDistance}: ${(widget.path.distance / 1000).toStringAsFixed(2)} km',
              style: textTheme.bodyMedium?.copyWith(
                color: Theme.of(context).colorScheme.onSurfaceVariant,
              ),
            ),
            const SizedBox(height: 24),
            PrimaryButton(
              text: l10n.saveChanges,
              onPressed: _isLoading || _isDeleting ? null : _updatePath,
              isLoading: _isLoading,
            ),
            const SizedBox(height: 12),
            SecondaryButton(
              text: l10n.exportPath,
              onPressed: _isLoading || _isDeleting ? null : _showExportSheet,
            ),
            const SizedBox(height: 12),
            SecondaryButton(
              text: l10n.deletePath,
              onPressed: _isLoading || _isDeleting ? null : _confirmDelete,
            ),
          ],
        ),
      ),
    );
  }

  void _showExportSheet() {
    showModalBottomSheet(
      context: context,
      isScrollControlled: true,
      useSafeArea: true,
      shape: const RoundedRectangleBorder(
        borderRadius: BorderRadius.vertical(top: Radius.circular(16)),
      ),
      builder: (_) => ExportOptionsSheet(path: widget.path),
    );
  }

  void _confirmDelete() {
    final l10n = context.l10n;
    showDialog(
      context: context,
      builder: (BuildContext dialogContext) {
        return AlertDialog.adaptive(
          title: Text(l10n.confirmDeletePathTitle),
          content: Text(l10n.confirmDeleteMessage),
          actions: [
            TextButton(
              onPressed: () => Navigator.of(dialogContext).pop(),
              child: Text(l10n.cancel),
            ),
            FilledButton(
              style: FilledButton.styleFrom(
                backgroundColor: Theme.of(context).colorScheme.error,
                foregroundColor: Theme.of(context).colorScheme.onError,
              ),
              onPressed: () {
                Navigator.of(dialogContext).pop();
                _deletePath();
              },
              child: Text(l10n.delete),
            ),
          ],
        );
      },
    );
  }

  Future<void> _updatePath() async {
    final l10n = context.l10n;
    if (!_formKey.currentState!.validate()) return;

    setState(() => _isLoading = true);
    try {
      final updatedPath = widget.path.copyWith(
        title: _nameController.text.trim(),
        description: _descriptionController.text.trim().isEmpty
            ? null
            : _descriptionController.text.trim(),
        colorHex: _selectedColor != null ? colorToHex(_selectedColor!) : null,
        clearColorHex: _selectedColor == null,
        iconKey: _selectedIconKey,
        clearIconKey: _selectedIconKey == null,
        smoothing: _isSmoothing,
        lineStyleKey: _lineStyle == PathLineStyle.solid ? null : _lineStyle.key,
        clearLineStyleKey: _lineStyle == PathLineStyle.solid,
      );

      await ref.read(savedPathRepositoryProvider.notifier).updatePath(updatedPath);

      if (mounted) {
        Navigator.of(context).pop(PathDetailResult.updated);
      }
    } catch (error) {
      if (mounted) {
        _showErrorSnackBar(context, l10n.errorSavingPath(error.toString()));
      }
    } finally {
      if (mounted) {
        setState(() => _isLoading = false);
      }
    }
  }

  Future<void> _deletePath() async {
    final l10n = context.l10n;
    setState(() => _isDeleting = true);
    try {
      await ref.read(savedPathRepositoryProvider.notifier).deletePath(widget.path.uuid);
      if (mounted) {
        Navigator.of(context).pop(PathDetailResult.deleted);
      }
    } catch (error) {
      if (mounted) {
        _showErrorSnackBar(context, l10n.errorDeletingPath(error.toString()));
      }
    } finally {
      if (mounted) {
        setState(() => _isDeleting = false);
      }
    }
  }

  void _showErrorSnackBar(BuildContext context, String message) {
    final colorScheme = Theme.of(context).colorScheme;
    ScaffoldMessenger.of(context).showSnackBar(
      SnackBar(
        content: Text(message),
        behavior: SnackBarBehavior.floating,
        backgroundColor: colorScheme.errorContainer,
        shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(12)),
        margin: const EdgeInsets.all(16),
      ),
    );
  }
}
