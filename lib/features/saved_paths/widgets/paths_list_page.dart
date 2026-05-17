import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:turbo/app/l10n/app_localizations.dart';
import 'package:turbo/core/widgets/app_snackbars.dart';
import 'package:turbo/features/markers/api.dart' show IconService;

import '../data/saved_path_repository.dart';
import '../models/path_style.dart';
import '../models/saved_path.dart';
import 'path_detail_sheet.dart';
import 'path_info_sheet.dart';

class PathsListPage extends ConsumerWidget {
  const PathsListPage({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final l10n = context.l10n;
    final asyncPaths = ref.watch(savedPathRepositoryProvider);
    final colorScheme = Theme.of(context).colorScheme;
    final iconService = IconService();

    return Scaffold(
      appBar: AppBar(title: Text(l10n.allPaths)),
      body: asyncPaths.when(
        loading: () => const Center(child: CircularProgressIndicator()),
        error: (e, _) => Center(child: Text('$e')),
        data: (paths) {
          if (paths.isEmpty) {
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
          final sorted = [...paths]
            ..sort((a, b) => b.createdAt.compareTo(a.createdAt));
          return ListView.separated(
            itemCount: sorted.length,
            separatorBuilder: (_, _) => const Divider(height: 1),
            itemBuilder: (context, i) {
              final p = sorted[i];
              final color = hexToColor(p.colorHex) ?? colorScheme.onSurfaceVariant;
              final namedIcon = p.iconKey != null
                  ? iconService.getIcon(context, p.iconKey)
                  : null;
              return ListTile(
                leading: CircleAvatar(
                  backgroundColor: color.withAlpha(40),
                  child: Icon(namedIcon?.icon ?? Icons.route, color: color),
                ),
                title: Text(p.title),
                subtitle: Text('${(p.distance / 1000).toStringAsFixed(2)} km'),
                onTap: () => _openInfo(context, p),
              );
            },
          );
        },
      ),
    );
  }

  Future<void> _openInfo(BuildContext context, SavedPath path) async {
    final l10n = context.l10n;
    final result = await showModalBottomSheet<PathDetailResult>(
      context: context,
      isScrollControlled: true,
      useSafeArea: true,
      builder: (_) => PathInfoSheet(path: path),
    );
    if (!context.mounted) return;
    if (result == PathDetailResult.deleted) {
      AppSnackbars.success(context, l10n.pathDeleted);
    } else if (result == PathDetailResult.updated) {
      AppSnackbars.success(context, l10n.pathUpdated);
    }
  }
}
