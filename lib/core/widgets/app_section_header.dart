import 'package:flutter/material.dart';
import 'package:turbo/app/tokens.dart';

/// Section header inside a scrollable page or sheet body.
///
/// Canonical style: `titleMedium` + `onSurfaceVariant` + `w600`, with 8dp
/// padding-below. Migrating to this style intentionally tones down the
/// previous settings-page "primary-colored bold" headers.
class AppSectionHeader extends StatelessWidget {
  final String title;

  const AppSectionHeader(this.title, {super.key});

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.only(bottom: AppSpacing.s),
      child: Text(
        title,
        style: Theme.of(context).textTheme.titleMedium?.copyWith(
              color: Theme.of(context).colorScheme.onSurfaceVariant,
              fontWeight: FontWeight.w600,
            ),
      ),
    );
  }
}
