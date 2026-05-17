import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:turbo/app/l10n/app_localizations.dart';
import 'package:turbo/core/widgets/app_button.dart';
import 'package:turbo/core/widgets/app_snackbars.dart';
import 'package:turbo/features/markers/api.dart';

/// Bottom sheet shown when the app opens an incoming `/share/m` link.
/// Lets the user preview the shared marker and import it into their own
/// collection.
class SharedMarkerPreviewSheet extends ConsumerWidget {
  final Marker marker;

  const SharedMarkerPreviewSheet({super.key, required this.marker});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final l10n = context.l10n;
    final theme = Theme.of(context);
    final mediaQuery = MediaQuery.of(context);
    final bottomPadding = mediaQuery.viewInsets.bottom > 0
        ? mediaQuery.viewInsets.bottom
        : mediaQuery.padding.bottom;

    final lat = marker.position.latitude.toStringAsFixed(6);
    final lon = marker.position.longitude.toStringAsFixed(6);

    return Padding(
      padding: EdgeInsets.fromLTRB(16, 16, 16, 16 + bottomPadding),
      child: Column(
        mainAxisSize: MainAxisSize.min,
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          Row(
            mainAxisAlignment: MainAxisAlignment.spaceBetween,
            children: [
              Text(l10n.sharedMarkerTitle, style: theme.textTheme.titleLarge),
              IconButton(
                onPressed: () => Navigator.pop(context),
                icon: const Icon(Icons.close),
                tooltip: l10n.close,
              ),
            ],
          ),
          const SizedBox(height: 8),
          Text(marker.title, style: theme.textTheme.headlineSmall),
          if (marker.description != null && marker.description!.isNotEmpty) ...[
            const SizedBox(height: 8),
            Text(marker.description!, style: theme.textTheme.bodyMedium),
          ],
          const SizedBox(height: 12),
          Text(
            '$lat, $lon',
            style: theme.textTheme.bodySmall?.copyWith(
              color: theme.colorScheme.onSurfaceVariant,
            ),
          ),
          const SizedBox(height: 24),
          AppButton.primary(
            text: l10n.saveToMyMarkers,
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
    // Copy the marker without the shared uuid so the local store assigns a
    // fresh one — otherwise two recipients of the same share would clash.
    final fresh = Marker(
      title: marker.title,
      description: marker.description,
      icon: marker.icon,
      position: marker.position,
    );
    await ref.read(locationRepositoryProvider.notifier).addMarker(fresh);
    if (!context.mounted) return;
    navigator.pop();
    AppSnackbars.success(context, l10n.savedToMyMarkers);
  }
}
