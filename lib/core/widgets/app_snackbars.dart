import 'package:flutter/material.dart';
import 'package:turbo/app/tokens.dart';

/// Centralized snackbar styling. Three flavors:
///
/// - [success]: pill (StadiumBorder), `primaryContainer`, leading check icon.
/// - [error]: rounded rectangle, `errorContainer`.
/// - [info]: rounded rectangle, theme default (neutral container).
class AppSnackbars {
  static void success(BuildContext context, String message) {
    final colorScheme = Theme.of(context).colorScheme;
    ScaffoldMessenger.of(context).showSnackBar(
      SnackBar(
        content: Row(
          children: [
            Icon(Icons.check_circle,
                color: colorScheme.onPrimaryContainer, size: 20),
            const SizedBox(width: AppSpacing.s),
            Expanded(
              child: Text(
                message,
                style: Theme.of(context)
                    .textTheme
                    .bodyMedium
                    ?.copyWith(color: colorScheme.onPrimaryContainer),
              ),
            ),
          ],
        ),
        backgroundColor: colorScheme.primaryContainer,
        behavior: SnackBarBehavior.floating,
        shape: const StadiumBorder(),
        margin: const EdgeInsets.all(AppSpacing.l),
      ),
    );
  }

  static void error(BuildContext context, String message) {
    final colorScheme = Theme.of(context).colorScheme;
    ScaffoldMessenger.of(context).showSnackBar(
      SnackBar(
        content: Text(message),
        backgroundColor: colorScheme.errorContainer,
        behavior: SnackBarBehavior.floating,
        margin: const EdgeInsets.all(AppSpacing.l),
      ),
    );
  }

  static void info(BuildContext context, String message) {
    ScaffoldMessenger.of(context).showSnackBar(
      SnackBar(
        content: Text(message),
        behavior: SnackBarBehavior.floating,
        margin: const EdgeInsets.all(AppSpacing.l),
      ),
    );
  }
}
