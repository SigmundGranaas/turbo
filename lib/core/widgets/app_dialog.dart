import 'package:flutter/material.dart';
import 'package:turbo/app/l10n/app_localizations.dart';

/// Confirmation dialogs. Both routes through `AlertDialog.adaptive` so iOS
/// gets Cupertino chrome automatically.
///
/// Resolves the latent ordering bug in `offline_regions_page.dart` where the
/// caller fired `deleteRegion` *before* popping — these helpers always pop
/// first and return a bool; the caller awaits and acts.
class AppDialog {
  /// Neutral yes/no confirmation.
  static Future<bool> confirm(
    BuildContext context, {
    required String title,
    required String content,
    String? confirmLabel,
    String? cancelLabel,
  }) async {
    final l10n = AppLocalizations.of(context);
    final result = await showDialog<bool>(
      context: context,
      builder: (dialogContext) => AlertDialog.adaptive(
        title: Text(title),
        content: Text(content),
        actions: [
          TextButton(
            onPressed: () => Navigator.of(dialogContext).pop(false),
            child: Text(cancelLabel ?? l10n.cancel),
          ),
          FilledButton(
            onPressed: () => Navigator.of(dialogContext).pop(true),
            child: Text(confirmLabel ?? l10n.ok),
          ),
        ],
      ),
    );
    return result ?? false;
  }

  /// Destructive (red) confirmation. The "confirm" button uses the error
  /// color scheme. Cancel is a TextButton.
  static Future<bool> destructive(
    BuildContext context, {
    required String title,
    required String content,
    required String destructiveLabel,
    String? cancelLabel,
  }) async {
    final l10n = AppLocalizations.of(context);
    final colorScheme = Theme.of(context).colorScheme;
    final result = await showDialog<bool>(
      context: context,
      builder: (dialogContext) => AlertDialog.adaptive(
        title: Text(title),
        content: Text(content),
        actions: [
          TextButton(
            onPressed: () => Navigator.of(dialogContext).pop(false),
            child: Text(cancelLabel ?? l10n.cancel),
          ),
          FilledButton(
            style: FilledButton.styleFrom(
              backgroundColor: colorScheme.error,
              foregroundColor: colorScheme.onError,
            ),
            onPressed: () => Navigator.of(dialogContext).pop(true),
            child: Text(destructiveLabel),
          ),
        ],
      ),
    );
    return result ?? false;
  }
}
