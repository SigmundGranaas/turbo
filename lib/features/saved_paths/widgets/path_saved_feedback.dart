import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import 'package:turbo/app/l10n/app_localizations.dart';
import 'package:turbo/app/tokens.dart';

import '../data/data_visibility_provider.dart';
import '../data/saved_path_repository.dart';
import '../models/saved_path.dart';

/// Shared post-save UX for any path that just hit the repository — recording,
/// measuring, or paste-from-link. Forces the saved-paths layer visible so the
/// user actually sees the trip they just persisted, then shows a snackbar
/// with an Undo action that deletes the path again. Without this, a saved
/// path can silently disappear into the list if the user previously toggled
/// the layer off, or be impossible to undo via the UI.
void showPathSavedFeedback(
  BuildContext context,
  WidgetRef ref,
  SavedPath path,
) {
  // Make the layer visible (no-op if already on).
  ref.read(savedPathsVisibleProvider.notifier).setVisible(true);

  final l10n = AppLocalizations.of(context);
  final colorScheme = Theme.of(context).colorScheme;
  final messenger = ScaffoldMessenger.maybeOf(context);
  if (messenger == null) return;

  messenger.hideCurrentSnackBar();
  messenger.showSnackBar(
    SnackBar(
      content: Row(
        children: [
          Icon(Icons.check_circle,
              color: colorScheme.onPrimaryContainer, size: 20),
          const SizedBox(width: AppSpacing.s),
          Expanded(
            child: Text(
              l10n.pathSavedNamed(path.title),
              style: Theme.of(context).textTheme.bodyMedium?.copyWith(
                    color: colorScheme.onPrimaryContainer,
                  ),
            ),
          ),
        ],
      ),
      backgroundColor: colorScheme.primaryContainer,
      behavior: SnackBarBehavior.floating,
      shape: const StadiumBorder(),
      margin: const EdgeInsets.all(AppSpacing.l),
      duration: const Duration(seconds: 6),
      action: SnackBarAction(
        label: l10n.undo,
        textColor: colorScheme.onPrimaryContainer,
        onPressed: () {
          ref
              .read(savedPathRepositoryProvider.notifier)
              .deletePath(path.uuid);
        },
      ),
    ),
  );
}
