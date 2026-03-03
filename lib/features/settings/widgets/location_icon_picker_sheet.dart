import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:image_picker/image_picker.dart';

import 'package:turbo/core/widgets/color_circle.dart';
import 'package:turbo/features/markers/widgets/icon_selection_page.dart';
import 'package:turbo/features/saved_paths/models/path_style.dart';
import 'package:turbo/features/settings/data/settings_provider.dart';
import 'package:turbo/l10n/app_localizations.dart';

/// Shows the location icon picker bottom sheet.
///
/// Used from both the Settings page and the current location marker tap.
void showLocationIconPickerSheet(BuildContext context, WidgetRef ref) {
  showModalBottomSheet(
    context: context,
    isScrollControlled: true,
    useSafeArea: true,
    builder: (sheetContext) => _LocationIconPickerContent(
      ref: ref,
      navigatorContext: context,
    ),
  );
}

class _LocationIconPickerContent extends ConsumerWidget {
  final WidgetRef ref;
  final BuildContext navigatorContext;

  const _LocationIconPickerContent({
    required this.ref,
    required this.navigatorContext,
  });

  @override
  Widget build(BuildContext context, WidgetRef widgetRef) {
    final l10n = AppLocalizations.of(context);
    final textTheme = Theme.of(context).textTheme;
    final mediaQuery = MediaQuery.of(context);
    final bottomPadding = mediaQuery.viewInsets.bottom > 0
        ? mediaQuery.viewInsets.bottom
        : mediaQuery.padding.bottom;

    final settings = widgetRef.watch(settingsProvider).value;
    final showHeading = settings?.showHeadingArrow ?? false;
    final arrowColorHex = settings?.markerArrowColorHex;
    final outlineColorHex = settings?.markerOutlineColorHex;

    return Padding(
      padding: EdgeInsets.fromLTRB(16, 16, 16, 16 + bottomPadding),
      child: Column(
        mainAxisSize: MainAxisSize.min,
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          // Header
          Row(
            mainAxisAlignment: MainAxisAlignment.spaceBetween,
            children: [
              Text(l10n.locationIcon, style: textTheme.titleLarge),
              IconButton(
                onPressed: () => Navigator.pop(context),
                icon: const Icon(Icons.close),
              ),
            ],
          ),
          const SizedBox(height: 16),

          // Choose Icon option
          ListTile(
            leading: const Icon(Icons.grid_view),
            title: Text(l10n.chooseIcon),
            shape: RoundedRectangleBorder(
              borderRadius: BorderRadius.circular(12),
            ),
            onTap: () async {
              Navigator.pop(context);
              final icon = await IconSelectionPage.show(navigatorContext);
              if (icon != null) {
                ref
                    .read(settingsProvider.notifier)
                    .setLocationBuiltinIcon(icon.title);
              }
            },
          ),

          // Choose from Gallery option
          ListTile(
            leading: const Icon(Icons.photo_library),
            title: Text(l10n.chooseFromGallery),
            shape: RoundedRectangleBorder(
              borderRadius: BorderRadius.circular(12),
            ),
            onTap: () async {
              Navigator.pop(context);
              final picker = ImagePicker();
              final image =
                  await picker.pickImage(source: ImageSource.gallery);
              if (image != null) {
                ref
                    .read(settingsProvider.notifier)
                    .setLocationImage(image.path);
              }
            },
          ),

          // Take Photo option
          ListTile(
            leading: const Icon(Icons.camera_alt),
            title: Text(l10n.takePhoto),
            shape: RoundedRectangleBorder(
              borderRadius: BorderRadius.circular(12),
            ),
            onTap: () async {
              Navigator.pop(context);
              final picker = ImagePicker();
              final image =
                  await picker.pickImage(source: ImageSource.camera);
              if (image != null) {
                ref
                    .read(settingsProvider.notifier)
                    .setLocationImage(image.path);
              }
            },
          ),
          const SizedBox(height: 16),

          // Arrow color picker (only when heading arrow is enabled)
          if (showHeading) ...[
            _buildColorPickerCard(
              context: context,
              label: l10n.arrowColor,
              selectedHex: arrowColorHex,
              onColorChanged: (color) {
                ref.read(settingsProvider.notifier).setMarkerArrowColor(color);
              },
            ),
            const SizedBox(height: 12),
          ],

          // Outline color picker
          _buildColorPickerCard(
            context: context,
            label: l10n.outlineColor,
            selectedHex: outlineColorHex,
            onColorChanged: (color) {
              ref.read(settingsProvider.notifier).setMarkerOutlineColor(color);
            },
          ),
          const SizedBox(height: 16),

          // Reset to Default
          TextButton(
            onPressed: () {
              Navigator.pop(context);
              ref.read(settingsProvider.notifier).resetLocationIcon();
            },
            child: Text(l10n.resetToDefault),
          ),
        ],
      ),
    );
  }

  Widget _buildColorPickerCard({
    required BuildContext context,
    required String label,
    required String? selectedHex,
    required ValueChanged<Color?> onColorChanged,
  }) {
    final colorScheme = Theme.of(context).colorScheme;
    final textTheme = Theme.of(context).textTheme;
    final l10n = AppLocalizations.of(context);

    return Card(
      elevation: 0,
      color: colorScheme.surfaceContainerLow,
      shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(12)),
      child: Padding(
        padding: const EdgeInsets.all(12),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Text(label, style: textTheme.bodyLarge),
            const SizedBox(height: 8),
            SingleChildScrollView(
              scrollDirection: Axis.horizontal,
              child: Row(
                children: [
                  ColorCircle(
                    color: null,
                    isSelected: selectedHex == null,
                    onTap: () => onColorChanged(null),
                    label: l10n.defaultColor,
                    colorScheme: colorScheme,
                  ),
                  ...pathColorPalette.map((color) => ColorCircle(
                        color: color,
                        isSelected: selectedHex != null &&
                            selectedHex == colorToHex(color),
                        onTap: () => onColorChanged(color),
                        colorScheme: colorScheme,
                      )),
                ],
              ),
            ),
          ],
        ),
      ),
    );
  }
}

