import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:latlong2/latlong.dart';
import 'package:turbo/core/widgets/app_button.dart';
import 'package:turbo/core/widgets/app_snackbars.dart';
import 'package:turbo/core/widgets/app_text_field.dart';
import 'package:turbo/l10n/app_localizations.dart';
import '../models/saved_path.dart';
import '../models/path_style.dart';
import '../data/saved_path_repository.dart';
import 'path_customization_controls.dart';

class SavePathSheet extends ConsumerStatefulWidget {
  final List<LatLng> points;
  final double distance;
  final bool isSmoothing;

  const SavePathSheet({
    super.key,
    required this.points,
    required this.distance,
    this.isSmoothing = false,
  });

  @override
  ConsumerState<SavePathSheet> createState() => _SavePathSheetState();
}

class _SavePathSheetState extends ConsumerState<SavePathSheet> {
  final _formKey = GlobalKey<FormState>();
  late final TextEditingController _nameController;
  late final TextEditingController _descriptionController;
  bool _isLoading = false;
  bool _showDescription = false;
  Color? _selectedColor;
  String? _selectedIconKey;
  late bool _isSmoothing;
  PathLineStyle _lineStyle = PathLineStyle.solid;

  @override
  void initState() {
    super.initState();
    _nameController = TextEditingController();
    _descriptionController = TextEditingController();
    _isSmoothing = widget.isSmoothing;
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
                Text(l10n.savePath, style: textTheme.titleLarge),
                IconButton(
                  onPressed: () => Navigator.pop(context, false),
                  icon: const Icon(Icons.close),
                ),
              ],
            ),
            const SizedBox(height: 24),
            AppTextField(
              controller: _nameController,
              label: l10n.pathName,
              autofocus: true,
              validator: (value) {
                if (value == null || value.trim().isEmpty) {
                  return l10n.pleaseEnterName;
                }
                return null;
              },
            ),
            const SizedBox(height: 16),
            if (_showDescription)
              AppTextField(
                controller: _descriptionController,
                label: l10n.descriptionOptional,
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
              initiallyExpanded: false,
            ),
            const SizedBox(height: 16),
            Text(
              '${l10n.totalDistance}: ${(widget.distance / 1000).toStringAsFixed(2)} km',
              style: textTheme.bodyMedium?.copyWith(
                color: Theme.of(context).colorScheme.onSurfaceVariant,
              ),
            ),
            const SizedBox(height: 24),
            AppButton.primary(
              text: l10n.savePath,
              onPressed: _isLoading ? null : _savePath,
              isLoading: _isLoading,
              fullWidth: true,
            ),
            const SizedBox(height: 12),
            AppButton.secondary(
              text: l10n.discardPath,
              onPressed: _isLoading ? null : () => Navigator.pop(context, false),
              fullWidth: true,
            ),
          ],
        ),
      ),
    );
  }

  void _savePath() async {
    final l10n = context.l10n;
    if (!_formKey.currentState!.validate()) return;

    setState(() => _isLoading = true);

    try {
      final path = SavedPath(
        title: _nameController.text.trim(),
        description: _descriptionController.text.trim().isEmpty
            ? null
            : _descriptionController.text.trim(),
        points: widget.points,
        distance: widget.distance,
        colorHex: _selectedColor != null ? colorToHex(_selectedColor!) : null,
        iconKey: _selectedIconKey,
        smoothing: _isSmoothing,
        lineStyleKey: _lineStyle == PathLineStyle.solid ? null : _lineStyle.key,
      );

      await ref.read(savedPathRepositoryProvider.notifier).addPath(path);

      if (!mounted) return;
      Navigator.of(context).pop(true);
    } catch (e) {
      if (!mounted) return;
      _showErrorSnackBar(context, l10n.errorSavingPath(e.toString()));
    } finally {
      if (mounted) {
        setState(() => _isLoading = false);
      }
    }
  }

  void _showErrorSnackBar(BuildContext context, String message) {
    AppSnackbars.error(context, message);
  }
}
