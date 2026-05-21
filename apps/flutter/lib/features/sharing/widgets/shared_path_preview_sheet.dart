import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:turbo/app/l10n/app_localizations.dart';
import 'package:turbo/core/widgets/app_button.dart';
import 'package:turbo/core/widgets/app_snackbars.dart';
import 'package:turbo/features/saved_paths/api.dart';
import 'package:turbo/features/settings/api.dart';

/// Bottom sheet shown when the app opens an incoming `/share/p` link.
class SharedPathPreviewSheet extends ConsumerWidget {
  final SavedPath path;

  const SharedPathPreviewSheet({super.key, required this.path});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final l10n = context.l10n;
    final theme = Theme.of(context);
    final unit = ref.watch(settingsProvider
        .select((s) => s.value?.distanceUnit ?? DistanceUnit.metric));
    final mediaQuery = MediaQuery.of(context);
    final bottomPadding = mediaQuery.viewInsets.bottom > 0
        ? mediaQuery.viewInsets.bottom
        : mediaQuery.padding.bottom;

    return Padding(
      padding: EdgeInsets.fromLTRB(16, 16, 16, 16 + bottomPadding),
      child: Column(
        mainAxisSize: MainAxisSize.min,
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          Row(
            mainAxisAlignment: MainAxisAlignment.spaceBetween,
            children: [
              Text(l10n.sharedPathTitle, style: theme.textTheme.titleLarge),
              IconButton(
                onPressed: () => Navigator.pop(context),
                icon: const Icon(Icons.close),
                tooltip: l10n.close,
              ),
            ],
          ),
          const SizedBox(height: 8),
          Text(path.title, style: theme.textTheme.headlineSmall),
          if (path.description != null && path.description!.isNotEmpty) ...[
            const SizedBox(height: 8),
            Text(path.description!, style: theme.textTheme.bodyMedium),
          ],
          const SizedBox(height: 12),
          Row(
            children: [
              Icon(Icons.straighten,
                  size: 16, color: theme.colorScheme.onSurfaceVariant),
              const SizedBox(width: 4),
              Text(
                formatDistance(path.distance, unit),
                style: theme.textTheme.bodySmall?.copyWith(
                  color: theme.colorScheme.onSurfaceVariant,
                ),
              ),
              const SizedBox(width: 16),
              Icon(Icons.scatter_plot,
                  size: 16, color: theme.colorScheme.onSurfaceVariant),
              const SizedBox(width: 4),
              Text(
                l10n.pointCount(path.points.length),
                style: theme.textTheme.bodySmall?.copyWith(
                  color: theme.colorScheme.onSurfaceVariant,
                ),
              ),
            ],
          ),
          const SizedBox(height: 24),
          AppButton.primary(
            text: l10n.saveToMyPaths,
            icon: Icons.bookmark_add,
            fullWidth: true,
            onPressed: () => _save(context, ref),
          ),
        ],
      ),
    );
  }

  Future<void> _save(BuildContext context, WidgetRef ref) async {
    final l10n = context.l10n;
    final navigator = Navigator.of(context);
    final fresh = SavedPath(
      title: path.title,
      description: path.description,
      points: path.points,
      distance: path.distance,
      colorHex: path.colorHex,
      iconKey: path.iconKey,
      smoothing: path.smoothing,
      lineStyleKey: path.lineStyleKey,
    );
    await ref.read(savedPathRepositoryProvider.notifier).addPath(fresh);
    if (!context.mounted) return;
    navigator.pop();
    AppSnackbars.success(context, l10n.savedToMyPaths);
  }
}
