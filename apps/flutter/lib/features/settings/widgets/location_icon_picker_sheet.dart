import 'package:flutter/material.dart';
import 'package:turbo/core/widgets/exclusive_sheet.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:image_picker/image_picker.dart';

import 'package:turbo/app/tokens.dart';
import 'package:turbo/core/widgets/app_list_card.dart';
import 'package:turbo/features/markers/api.dart';
import 'package:turbo/features/settings/data/settings_provider.dart';
import 'package:turbo/app/l10n/app_localizations.dart';

/// Shows the location-icon picker bottom sheet.
///
/// Opened from the Settings page and from tapping the current location dot.
///
/// The sheet exposes four flat options — Choose icon / Gallery / Take photo /
/// Reset — directly as [AppListCard] rows. The previous design used a row
/// of small `ActionButton` tiles that each opened *another* sheet (icon source
/// or colors). The colors flow was redundant because color pickers are
/// already shown directly on the settings page below the icon row, and the
/// nested sheets made the affordance feel hidden.
void showLocationIconPickerSheet(BuildContext context, WidgetRef ref) {
  showExclusiveSheet(
    context,
    builder: (_) => _LocationIconPickerSheet(
      ref: ref,
      navigatorContext: context,
    ),
  );
}

class _LocationIconPickerSheet extends ConsumerWidget {
  final WidgetRef ref;
  final BuildContext navigatorContext;

  const _LocationIconPickerSheet({
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

    return Padding(
      padding: EdgeInsets.fromLTRB(
        AppSpacing.l,
        AppSpacing.l,
        AppSpacing.l,
        AppSpacing.l + bottomPadding,
      ),
      child: Column(
        mainAxisSize: MainAxisSize.min,
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
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
          const SizedBox(height: AppSpacing.l),

          AppListCard(
            icon: Icons.grid_view,
            title: l10n.chooseIcon,
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
          const SizedBox(height: AppSpacing.m),
          AppListCard(
            icon: Icons.photo_library,
            title: l10n.chooseFromGallery,
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
          const SizedBox(height: AppSpacing.m),
          AppListCard(
            icon: Icons.camera_alt,
            title: l10n.takePhoto,
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
          const SizedBox(height: AppSpacing.m),
          AppListCard(
            icon: Icons.restart_alt,
            title: l10n.resetToDefault,
            onTap: () {
              Navigator.pop(context);
              ref.read(settingsProvider.notifier).resetLocationIcon();
            },
          ),
        ],
      ),
    );
  }
}
