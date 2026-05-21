import 'package:sqflite/sqflite.dart';
import 'package:turbo/core/data/database_provider.dart';

import '../models/activity_geometry.dart';
import '../models/activity_summary.dart';

/// Local persistence for the cross-kind activity summaries read model.
/// Mirrors the server's `/api/activities/summaries/*` shape so the map
/// can paint pins on cold start (and survive transient offline) before
/// the delta-sync round-trip lands.
abstract class ActivitySummaryStore {
  Future<List<ActivitySummary>> getAll();
  Future<void> upsertMany(List<ActivitySummary> items);
  Future<void> upsert(ActivitySummary item);
  Future<void> deleteMany(List<String> ids);
  Future<void> remove(String id);
  Future<void> clearAll();
}

class SqliteActivitySummaryStore implements ActivitySummaryStore {
  final Database _db;
  SqliteActivitySummaryStore(this._db);

  @override
  Future<List<ActivitySummary>> getAll() async {
    final rows = await _db.query(activitySummariesTable);
    return rows.map(_fromRow).toList();
  }

  @override
  Future<void> upsertMany(List<ActivitySummary> items) async {
    if (items.isEmpty) return;
    final batch = _db.batch();
    for (final s in items) {
      batch.insert(
        activitySummariesTable,
        _toRow(s),
        conflictAlgorithm: ConflictAlgorithm.replace,
      );
    }
    await batch.commit(noResult: true);
  }

  @override
  Future<void> upsert(ActivitySummary item) async {
    await _db.insert(
      activitySummariesTable,
      _toRow(item),
      conflictAlgorithm: ConflictAlgorithm.replace,
    );
  }

  @override
  Future<void> deleteMany(List<String> ids) async {
    if (ids.isEmpty) return;
    final batch = _db.batch();
    for (final id in ids) {
      batch.delete(activitySummariesTable, where: 'id = ?', whereArgs: [id]);
    }
    await batch.commit(noResult: true);
  }

  @override
  Future<void> remove(String id) async {
    await _db.delete(activitySummariesTable, where: 'id = ?', whereArgs: [id]);
  }

  @override
  Future<void> clearAll() async {
    await _db.delete(activitySummariesTable);
  }

  Map<String, Object?> _toRow(ActivitySummary s) => {
        'id': s.id,
        'kind': s.kind,
        'name': s.name,
        'geometry_wkt': s.geometry.wkt,
        'geometry_kind': _geometryKindToWire(s.geometry.kind),
        'icon_key': s.iconKey,
        'color_hex': s.colorHex,
        'updated_at': s.updatedAt.toUtc().millisecondsSinceEpoch,
        'version': s.version,
      };

  ActivitySummary _fromRow(Map<String, Object?> row) {
    final wkt = row['geometry_wkt'] as String;
    final kindWire = row['geometry_kind'] as String;
    return ActivitySummary(
      id: row['id'] as String,
      kind: row['kind'] as String,
      name: row['name'] as String,
      geometry: ActivityGeometry.fromServer(wkt: wkt, geometryKind: kindWire),
      iconKey: row['icon_key'] as String,
      colorHex: row['color_hex'] as String?,
      updatedAt: DateTime.fromMillisecondsSinceEpoch(
          row['updated_at'] as int,
          isUtc: true),
      version: row['version'] as int,
    );
  }

  static String _geometryKindToWire(ActivityGeometryKind kind) => switch (kind) {
        ActivityGeometryKind.point => 'POINT',
        ActivityGeometryKind.lineString => 'LINESTRING',
        ActivityGeometryKind.polygon => 'POLYGON',
      };
}
