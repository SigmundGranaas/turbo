import 'package:flutter/material.dart';
import 'package:turbo/core/widgets/exclusive_sheet.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:turbo/app/l10n/app_localizations.dart';
import 'package:turbo/core/widgets/app_snackbars.dart';

import '../data/icon_service.dart';
import '../data/location_repository.dart';
import '../models/marker.dart';
import 'marker_info_sheet.dart';

class MarkersListPage extends ConsumerWidget {
  const MarkersListPage({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final l10n = context.l10n;
    final asyncMarkers = ref.watch(locationRepositoryProvider);
    final colorScheme = Theme.of(context).colorScheme;
    final iconService = IconService();

    return Scaffold(
      appBar: AppBar(title: Text(l10n.allMarkers)),
      body: asyncMarkers.when(
        loading: () => const Center(child: CircularProgressIndicator()),
        error: (_, _) => Center(child: Text(l10n.genericLoadError)),
        data: (markers) {
          if (markers.isEmpty) {
            return Center(
              child: Padding(
                padding: const EdgeInsets.all(32),
                child: Text(
                  l10n.noResultsFound,
                  style: Theme.of(context).textTheme.bodyMedium?.copyWith(
                        color: colorScheme.onSurfaceVariant,
                      ),
                ),
              ),
            );
          }
          final sorted = [...markers]
            ..sort((a, b) => a.title.toLowerCase().compareTo(b.title.toLowerCase()));
          return ListView.separated(
            itemCount: sorted.length,
            separatorBuilder: (_, _) => const Divider(height: 1),
            itemBuilder: (context, i) {
              final m = sorted[i];
              final namedIcon = iconService.getIcon(context, m.icon);
              return ListTile(
                leading: CircleAvatar(
                  backgroundColor: colorScheme.primaryContainer,
                  child: Icon(
                    namedIcon.icon,
                    color: colorScheme.onPrimaryContainer,
                  ),
                ),
                title: Text(m.title),
                subtitle: Text(
                  '${m.position.latitude.toStringAsFixed(4)}, '
                  '${m.position.longitude.toStringAsFixed(4)}',
                ),
                onTap: () => _openInfo(context, m),
              );
            },
          );
        },
      ),
    );
  }

  Future<void> _openInfo(BuildContext context, Marker marker) async {
    final l10n = context.l10n;
    final result = await showExclusiveSheet<MarkerInfoResult>(
      context,
      builder: (_) => MarkerInfoSheet(marker: marker),
    );
    if (!context.mounted) return;
    if (result == MarkerInfoResult.deleted) {
      AppSnackbars.success(context, l10n.markerDeleted);
    } else if (result == MarkerInfoResult.updated) {
      AppSnackbars.success(context, l10n.markerUpdated);
    }
  }
}
