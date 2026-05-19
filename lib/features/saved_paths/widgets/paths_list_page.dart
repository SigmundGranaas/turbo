import 'dart:io';

import 'package:file_picker/file_picker.dart';
import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:turbo/app/l10n/app_localizations.dart';
import 'package:turbo/core/widgets/app_snackbars.dart';
import 'package:turbo/features/markers/api.dart' show IconService;

import '../data/elevation_backfill.dart';
import '../data/path_importer.dart';
import '../data/path_journal_grouping.dart';
import '../data/saved_path_repository.dart';
import '../models/path_style.dart';
import '../models/saved_path.dart';
import 'path_detail_sheet.dart';
import 'path_info_sheet.dart';
import 'trip_stats_page.dart';

/// Filter applied to the journal list.
enum _PathFilter { all, recorded, imported }

/// Adapter that lets the data-layer grouping helper read from AppLocalizations
/// without coupling it to the BuildContext.
class _L10nLabels implements PathJournalLabels {
  final AppLocalizations _l;
  _L10nLabels(this._l);
  @override
  String get today => _l.pathsGroupToday;
  @override
  String get yesterday => _l.pathsGroupYesterday;
  @override
  String get thisWeek => _l.pathsGroupThisWeek;
  @override
  String get thisMonth => _l.pathsGroupThisMonth;
}

class PathsListPage extends ConsumerStatefulWidget {
  const PathsListPage({super.key});

  @override
  ConsumerState<PathsListPage> createState() => _PathsListPageState();
}

class _PathsListPageState extends ConsumerState<PathsListPage> {
  _PathFilter _filter = _PathFilter.all;

  @override
  Widget build(BuildContext context) {
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
        error: (_, _) => Center(child: Text(l10n.genericLoadError)),
        data: (paths) {
          final filtered = _applyFilter(paths, _filter);
          return Column(
            children: [
              _FilterChips(
                value: _filter,
                hasAny: paths.isNotEmpty,
                onChanged: (next) => setState(() => _filter = next),
              ),
              if (filtered.isEmpty)
                Expanded(
                  child: Center(
                    child: Padding(
                      padding: const EdgeInsets.all(32),
                      child: Text(
                        l10n.noResultsFound,
                        style: Theme.of(context)
                            .textTheme
                            .bodyMedium
                            ?.copyWith(color: colorScheme.onSurfaceVariant),
                      ),
                    ),
                  ),
                )
              else
                Expanded(
                  child: _buildJournalList(
                    filtered,
                    colorScheme,
                    iconService,
                    l10n,
                  ),
                ),
            ],
          );
        },
      ),
    );
  }

  static List<SavedPath> _applyFilter(
      List<SavedPath> paths, _PathFilter filter) {
    return switch (filter) {
      _PathFilter.all => paths,
      _PathFilter.recorded =>
        paths.where((p) => p.recordedAt != null).toList(),
      _PathFilter.imported =>
        paths.where((p) => p.recordedAt == null).toList(),
    };
  }

  Widget _buildJournalList(
    List<SavedPath> paths,
    ColorScheme colorScheme,
    IconService iconService,
    AppLocalizations l10n,
  ) {
    final now = DateTime.now();
    final labels = _L10nLabels(l10n);

    // Bucket each path. Use recordedAt for recorded paths (so they show up
    // "when the trip happened" rather than "when the row was inserted"); fall
    // back to createdAt for imported paths.
    final buckets = <String, _Bucket>{};
    for (final p in paths) {
      final when = p.recordedAt ?? p.createdAt;
      final group = groupForDate(when, now: now);
      final bucket = buckets.putIfAbsent(
        group.key,
        () => _Bucket(group: group, paths: [], header: group.headerBuilder(labels)),
      );
      bucket.paths.add(p);
    }

    final orderedBuckets = buckets.values.toList()
      ..sort((a, b) => a.group.order.compareTo(b.group.order));
    for (final bucket in orderedBuckets) {
      bucket.paths.sort((a, b) {
        final aWhen = a.recordedAt ?? a.createdAt;
        final bWhen = b.recordedAt ?? b.createdAt;
        return bWhen.compareTo(aWhen);
      });
    }

    // Flatten into a list of widgets so a single ListView handles scrolling.
    final children = <Widget>[];
    for (final bucket in orderedBuckets) {
      children.add(_SectionHeader(text: bucket.header));
      for (final p in bucket.paths) {
        children.add(_buildPathTile(p, colorScheme, iconService, l10n));
        children.add(const Divider(height: 1));
      }
    }

    return ListView(children: children);
  }

  Widget _buildPathTile(
    SavedPath p,
    ColorScheme colorScheme,
    IconService iconService,
    AppLocalizations l10n,
  ) {
    final color = hexToColor(p.colorHex) ?? colorScheme.onSurfaceVariant;
    final namedIcon =
        p.iconKey != null ? iconService.getIcon(context, p.iconKey) : null;
    final isRecorded = p.recordedAt != null;

    return ListTile(
      leading: CircleAvatar(
        backgroundColor: color.withAlpha(40),
        child: Icon(namedIcon?.icon ?? Icons.route, color: color),
      ),
      title: Text(p.title),
      subtitle: Row(
        children: [
          Text('${(p.distance / 1000).toStringAsFixed(2)} km'),
          if (isRecorded) ...[
            const SizedBox(width: 8),
            Container(
              padding: const EdgeInsets.symmetric(horizontal: 6, vertical: 2),
              decoration: BoxDecoration(
                color: colorScheme.primaryContainer,
                borderRadius: BorderRadius.circular(4),
              ),
              child: Row(
                mainAxisSize: MainAxisSize.min,
                children: [
                  Icon(Icons.fiber_manual_record,
                      size: 10, color: colorScheme.onPrimaryContainer),
                  const SizedBox(width: 3),
                  Text(
                    l10n.recordedBadge,
                    style: Theme.of(context).textTheme.labelSmall?.copyWith(
                          color: colorScheme.onPrimaryContainer,
                          fontWeight: FontWeight.w600,
                        ),
                  ),
                ],
              ),
            ),
          ],
        ],
      ),
      onTap: () => _openInfo(context, p),
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
    final backfill = ref.read(elevationBackfillServiceProvider);
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
        final result = await backfill.backfill(p);
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

class _Bucket {
  final PathJournalGroup group;
  final List<SavedPath> paths;
  final String header;
  _Bucket({required this.group, required this.paths, required this.header});
}

class _SectionHeader extends StatelessWidget {
  final String text;
  const _SectionHeader({required this.text});

  @override
  Widget build(BuildContext context) {
    final colorScheme = Theme.of(context).colorScheme;
    return Container(
      width: double.infinity,
      padding: const EdgeInsets.fromLTRB(16, 16, 16, 8),
      color: colorScheme.surfaceContainerLowest,
      child: Text(
        text,
        style: Theme.of(context).textTheme.labelLarge?.copyWith(
              color: colorScheme.primary,
              fontWeight: FontWeight.w600,
            ),
      ),
    );
  }
}

class _FilterChips extends StatelessWidget {
  final _PathFilter value;
  final bool hasAny;
  final ValueChanged<_PathFilter> onChanged;

  const _FilterChips({
    required this.value,
    required this.hasAny,
    required this.onChanged,
  });

  @override
  Widget build(BuildContext context) {
    if (!hasAny) return const SizedBox.shrink();
    final l10n = context.l10n;
    return Padding(
      padding: const EdgeInsets.fromLTRB(16, 12, 16, 4),
      child: Wrap(
        spacing: 8,
        children: [
          _chip(_PathFilter.all, l10n.pathsFilterAll),
          _chip(_PathFilter.recorded, l10n.pathsFilterRecorded),
          _chip(_PathFilter.imported, l10n.pathsFilterImported),
        ],
      ),
    );
  }

  Widget _chip(_PathFilter f, String label) {
    return ChoiceChip(
      label: Text(label),
      selected: value == f,
      onSelected: (_) => onChanged(f),
    );
  }
}

