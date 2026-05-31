import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:turbo/core/widgets/app_button.dart';
import 'package:turbo/core/widgets/app_snackbars.dart';
import 'package:turbo/core/widgets/app_text_field.dart';
import 'package:turbo/core/widgets/sheet_action_bar.dart';
import 'package:turbo/app/l10n/app_localizations.dart';
import 'package:turbo/features/activities/api.dart' as activities;
import 'package:turbo/features/settings/api.dart';
import 'package:turbo/features/sharing/api.dart';
import '../models/saved_path.dart';
import '../models/path_style.dart';
import '../data/saved_path_repository.dart';
import 'elevation_profile.dart';
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
            AppTextField(
              controller: _nameController,
              label: l10n.pathName,
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
              initiallyExpanded: true,
            ),
            const SizedBox(height: 16),
            _RecordingSummary(path: widget.path, lineColor: _selectedColor),
            Text(
              '${l10n.totalDistance}: ${(widget.path.distance / 1000).toStringAsFixed(2)} km',
              style: textTheme.bodyMedium?.copyWith(
                color: Theme.of(context).colorScheme.onSurfaceVariant,
              ),
            ),
            const SizedBox(height: 24),
            AppButton.primary(
              text: l10n.saveChanges,
              onPressed: _isLoading ? null : _updatePath,
              isLoading: _isLoading,
              fullWidth: true,
            ),
            const SizedBox(height: 8),
            // Secondary actions live in the shared bar beneath the primary
            // Save CTA. "Save as activity" hands this recorded route to the
            // activity kind picker (filtered to LineString kinds — hiking /
            // xc_ski / packrafting / backcountry_ski); Share is gated on the
            // platform actually supporting it.
            SheetActionBar(
              actions: [
                SheetAction(
                  icon: Icons.outdoor_grill_outlined,
                  label: l10n.saveAsActivity,
                  onPressed: _isLoading ? null : _promoteToActivity,
                ),
                if (ref.watch(sharingAvailableProvider))
                  SheetAction(
                    icon: Icons.share_outlined,
                    label: l10n.share,
                    onPressed: () => ShareSheet.show(
                      context,
                      widget.path.uuid,
                      title: widget.path.title,
                    ),
                  ),
              ],
            ),
          ],
        ),
      ),
    );
  }

  Future<void> _promoteToActivity() async {
    final points = widget.path.points;
    if (points.length < 2) return;
    final wkt = 'LINESTRING(${points.map((p) => '${p.longitude} ${p.latitude}').join(', ')})';
    final seed = activities.ActivityGeometry.fromServer(
      wkt: wkt,
      geometryKind: 'LINESTRING',
    );
    final saved = await showModalBottomSheet<bool>(
      context: context,
      isScrollControlled: true,
      useSafeArea: true,
      builder: (sheetCtx) => activities.ActivityCreatePicker(seedGeometry: seed),
    );
    // If the user actually saved a new activity, close this path sheet
    // too so they land on the map and see their new pin without
    // having to manually dismiss the sheet they came from.
    if (saved == true && mounted) Navigator.of(context).pop();
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

  void _showErrorSnackBar(BuildContext context, String message) {
    AppSnackbars.error(context, message);
  }
}

class _RecordingSummary extends ConsumerWidget {
  final SavedPath path;
  final Color? lineColor;

  const _RecordingSummary({required this.path, this.lineColor});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final theme = Theme.of(context);
    final unit = ref.watch(settingsProvider).value?.distanceUnit ?? DistanceUnit.metric;
    final elevations = path.elevations;
    final hasElevations = elevations != null && elevations.length >= 2;
    final ascent = path.ascent;
    final descent = path.descent;
    final movingSeconds = path.movingTimeSeconds;

    if (!hasElevations && ascent == null && descent == null && movingSeconds == null) {
      return const SizedBox.shrink();
    }

    final stats = <Widget>[];
    if (ascent != null) {
      stats.add(_StatChip(
        icon: Icons.trending_up,
        label: _elevationLabel(ascent, unit),
      ));
    }
    if (descent != null) {
      stats.add(_StatChip(
        icon: Icons.trending_down,
        label: _elevationLabel(descent, unit),
      ));
    }
    if (movingSeconds != null && movingSeconds > 0) {
      stats.add(_StatChip(
        icon: Icons.timer_outlined,
        label: _formatDuration(Duration(seconds: movingSeconds)),
      ));
    }

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        if (hasElevations) ...[
          ElevationProfile(
            elevations: elevations,
            unit: unit,
            lineColor: lineColor,
          ),
          const SizedBox(height: 8),
        ],
        if (stats.isNotEmpty)
          Wrap(
            spacing: 8,
            runSpacing: 4,
            children: stats,
          ),
        if (stats.isNotEmpty) const SizedBox(height: 12),
        if (!hasElevations && stats.isEmpty)
          const SizedBox.shrink()
        else
          Divider(color: theme.dividerColor.withValues(alpha: 0.4)),
      ],
    );
  }

  String _elevationLabel(double meters, DistanceUnit unit) {
    if (unit == DistanceUnit.imperial) {
      return '${(meters / 0.3048).round()} ft';
    }
    return '${meters.round()} m';
  }

  String _formatDuration(Duration d) {
    final h = d.inHours;
    final m = d.inMinutes.remainder(60);
    final s = d.inSeconds.remainder(60);
    if (h > 0) {
      return '$h:${m.toString().padLeft(2, '0')}:${s.toString().padLeft(2, '0')}';
    }
    return '$m:${s.toString().padLeft(2, '0')}';
  }
}

class _StatChip extends StatelessWidget {
  final IconData icon;
  final String label;

  const _StatChip({required this.icon, required this.label});

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 6),
      decoration: BoxDecoration(
        color: theme.colorScheme.surfaceContainerHighest,
        borderRadius: BorderRadius.circular(12),
      ),
      child: Row(
        mainAxisSize: MainAxisSize.min,
        children: [
          Icon(icon, size: 16, color: theme.colorScheme.onSurfaceVariant),
          const SizedBox(width: 4),
          Text(label, style: theme.textTheme.bodySmall),
        ],
      ),
    );
  }
}

