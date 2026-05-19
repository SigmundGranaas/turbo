import 'dart:io';

import 'package:file_picker/file_picker.dart';
import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:turbo/app/l10n/app_localizations.dart';
import 'package:turbo/core/widgets/app_snackbars.dart';
import 'package:turbo/features/markers/api.dart' show IconService;

import '../data/elevation_backfill.dart';
import '../data/hoydedata_service.dart';
import '../data/path_importer.dart';
import '../data/saved_path_repository.dart';
import '../models/path_style.dart';
import '../models/saved_path.dart';
import 'path_detail_sheet.dart';
import 'path_info_sheet.dart';
import 'trip_stats_page.dart';

class PathsListPage extends ConsumerWidget {
  const PathsListPage({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final l10n = context.l10n;
    final asyncPaths = ref.watch(savedPathRepositoryProvider);
    final colorScheme = Theme.of(context).colorScheme;
    final iconService = IconService();

    return Scaffold(
      appBar: AppBar(
        title: Text(l10n.allPaths),
        actions: [
          IconButton(
            tooltip: 'Trip statistics',
            icon: const Icon(Icons.insights_outlined),
            onPressed: () => Navigator.of(context).push(
              MaterialPageRoute(builder: (_) => const TripStatsPage()),
            ),
          ),
          IconButton(
            tooltip: 'Import GPX / GeoJSON / KML',
            icon: const Icon(Icons.file_download_outlined),
            onPressed: () => _importPaths(context, ref),
          ),
        ],
      ),
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

  Future<void> _importPaths(BuildContext context, WidgetRef ref) async {
    final messenger = ScaffoldMessenger.maybeOf(context);
    final l10n = context.l10n;
    FilePickerResult? picked;
    try {
      picked = await FilePicker.platform.pickFiles(
        type: FileType.custom,
        allowedExtensions: const ['gpx', 'geojson', 'json', 'kml'],
        withData: kIsWeb,
      );
    } catch (e) {
      messenger?.showSnackBar(SnackBar(content: Text('Could not open picker: $e')));
      return;
    }
    if (picked == null || picked.files.isEmpty) return;

    final file = picked.files.single;
    String content;
    try {
      if (kIsWeb) {
        final bytes = file.bytes;
        if (bytes == null) {
          messenger?.showSnackBar(
              const SnackBar(content: Text('Failed to read selected file.')));
          return;
        }
        content = String.fromCharCodes(bytes);
      } else {
        final path = file.path;
        if (path == null) {
          messenger?.showSnackBar(
              const SnackBar(content: Text('Failed to read selected file.')));
          return;
        }
        content = await File(path).readAsString();
      }
    } catch (e) {
      messenger?.showSnackBar(SnackBar(content: Text('Failed to read file: $e')));
      return;
    }

    List<SavedPath> parsed;
    try {
      parsed = importPathContent(content, filename: file.name);
    } on PathImportException catch (e) {
      messenger?.showSnackBar(SnackBar(content: Text(e.message)));
      return;
    } catch (e) {
      messenger?.showSnackBar(SnackBar(content: Text('Import failed: $e')));
      return;
    }

    if (parsed.isEmpty) {
      messenger?.showSnackBar(
          const SnackBar(content: Text('No tracks found in the file.')));
      return;
    }

    final repo = ref.read(savedPathRepositoryProvider.notifier);
    final service = HoydedataService();
    var anyBackfillFailed = false;
    var didBackfillToast = false;
    for (final p in parsed) {
      var toSave = p;
      // Most exported GPX files (including those produced by Strava and Garmin
      // exports of routes) come with elevation. Files exported from web-based
      // route planners often don't — for those we backfill from Kartverket
      // before persisting so the elevation profile renders end-to-end.
      final missingRatio = _missingElevationRatio(p);
      if (missingRatio >= 0.5 && p.points.length <= 2000) {
        if (!didBackfillToast) {
          didBackfillToast = true;
          messenger?.showSnackBar(SnackBar(
            content: Text(l10n.importBackfillingElevation),
            duration: const Duration(seconds: 2),
          ));
        }
        final result = await backfillElevations(p, service);
        if (result.status == ElevationBackfillStatus.failed) {
          anyBackfillFailed = true;
        } else {
          toSave = result.path;
        }
      }
      await repo.addPath(toSave);
    }
    if (anyBackfillFailed) {
      messenger?.showSnackBar(SnackBar(
        content: Text(l10n.importBackfillElevationFailed),
      ));
    }
    messenger?.showSnackBar(SnackBar(
      content: Text(parsed.length == 1
          ? 'Imported "${parsed.first.title}".'
          : 'Imported ${parsed.length} tracks.'),
    ));
  }

  static double _missingElevationRatio(SavedPath p) {
    if (p.points.isEmpty) return 0;
    final el = p.elevations;
    if (el == null) return 1;
    if (el.length < p.points.length) {
      return (p.points.length - el.length) / p.points.length;
    }
    var missing = 0;
    for (final v in el) {
      if (v.isNaN) missing++;
    }
    return missing / p.points.length;
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
