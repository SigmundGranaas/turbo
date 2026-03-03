import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:image_picker/image_picker.dart';

import 'package:turbo/features/markers/widgets/icon_selection_page.dart';
import 'package:turbo/features/settings/data/settings_provider.dart';
import 'package:turbo/l10n/app_localizations.dart';

/// Shows the location icon picker bottom sheet.
///
/// Used from both the Settings page and the current location marker tap.
void showLocationIconPickerSheet(BuildContext context, WidgetRef ref) {
  final l10n = AppLocalizations.of(context);
  showModalBottomSheet(
    context: context,
    builder: (sheetContext) => SafeArea(
      child: Column(
        mainAxisSize: MainAxisSize.min,
        children: [
          ListTile(
            leading: const Icon(Icons.grid_view),
            title: Text(l10n.chooseIcon),
            onTap: () async {
              Navigator.pop(sheetContext);
              final icon = await IconSelectionPage.show(context);
              if (icon != null) {
                ref
                    .read(settingsProvider.notifier)
                    .setLocationBuiltinIcon(icon.title);
              }
            },
          ),
          ListTile(
            leading: const Icon(Icons.photo_library),
            title: Text(l10n.chooseFromGallery),
            onTap: () async {
              Navigator.pop(sheetContext);
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
          ListTile(
            leading: const Icon(Icons.camera_alt),
            title: Text(l10n.takePhoto),
            onTap: () async {
              Navigator.pop(sheetContext);
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
          ListTile(
            leading: const Icon(Icons.refresh),
            title: Text(l10n.resetToDefault),
            onTap: () {
              Navigator.pop(sheetContext);
              ref.read(settingsProvider.notifier).resetLocationIcon();
            },
          ),
        ],
      ),
    ),
  );
}
