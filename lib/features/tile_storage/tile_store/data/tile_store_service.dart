import 'dart:collection';
import 'dart:io';
import 'package:flutter/foundation.dart';
import 'package:flutter_map/flutter_map.dart';
import 'package:logging/logging.dart';
import 'package:sqflite/sqflite.dart';
import 'package:turbo/features/tile_storage/tile_store/models/storage_stats.dart';
import 'package:path/path.dart' as p;
import 'package:path_provider/path_provider.dart';

import '../../../../core/data/database_provider.dart';


class TileStoreService {
  final Database _db;
  final _log = Logger('TileStoreService');
  
  // Simple mutex to prevent concurrent file writes to the same path
  final Set<String> _lockedPaths = {};

  // Public getter for the database instance
  Database get db => _db;

  // Configuration
  bool _enabled = true;
  int _maxMemoryItems = 500;

  // L1 In-Memory Cache (LRU)
  final LinkedHashMap<String, Uint8List> _memoryCache = LinkedHashMap();

  // L2 On-Disk Cache
  String? _tileFilesPath;

  /// Optional directory path for testing purposes, to avoid platform lookups.
  final String? testDirectory;

  TileStoreService(this._db, {this.testDirectory});

  Future<void> _withFileLock(String path, Future<void> Function() action) async {
    while (_lockedPaths.contains(path)) {
      await Future.delayed(const Duration(milliseconds: 10));
    }
    _lockedPaths.add(path);
    try {
      await action();
    } finally {
      _lockedPaths.remove(path);
    }
  }

  void configure({int? maxMemoryItems, bool? enabled}) {
    if (enabled != null) _enabled = enabled;
    if (maxMemoryItems != null) _maxMemoryItems = maxMemoryItems;
    if (_enabled) {
      _enforceMemoryPolicy();
    } else {
      clearMemoryCache();
    }
  }

  Future<Uint8List?> get(String providerId, TileCoordinates coords) async {
    if (!_enabled) return null;
    final key = _getTileKey(providerId, coords);

    // 1. Check L1 Memory Cache
    if (_memoryCache.containsKey(key)) {
      final data = _memoryCache.remove(key)!;
      _memoryCache[key] = data; // Mark as recently used
      return data;
    }

    // 2. Check L2 Disk Cache
    final fromDisk = await _getFromDisk(providerId, coords);
    if (fromDisk != null) {
      _putInMemory(key, fromDisk); // Populate L1 on disk hit
      return fromDisk;
    }
    return null; // Cache miss
  }

  Future<void> put(
      String providerId, TileCoordinates coords, Uint8List bytes) async {
    if (!_enabled) return;
    final key = _getTileKey(providerId, coords);
    _putInMemory(key, bytes);
    await _putOnDisk(providerId, coords, bytes);
  }

  /// Specialized put for offline downloads that sets the reference count immediately.
  Future<void> putWithReference(
      String providerId, TileCoordinates coords, Uint8List bytes) async {
    if (!_enabled) return;
    final key = _getTileKey(providerId, coords);
    _putInMemory(key, bytes);
    
    final path = await getTilePath(providerId, coords);
    
    await _withFileLock(path, () async {
      final file = File(path);
      try {
        await file.create(recursive: true);
        await file.writeAsBytes(bytes, flush: true);

        await _db.insert(
            tileStoreTable,
            {
              'providerId': providerId,
              'z': coords.z,
              'x': coords.x,
              'y': coords.y,
              'path': path,
              'sizeInBytes': bytes.length,
              'lastAccessed': DateTime.now().toIso8601String(),
              'referenceCount': 1,
            },
            conflictAlgorithm: ConflictAlgorithm.replace);
      } catch (e, s) {
        _log.warning('Failed to write tile to disk or DB. Path: $path', e, s);
      }
    });
  }

  Future<void> clearMemoryCache() async {
    _memoryCache.clear();
  }

  Future<int> clearDiskCache() async {
    final toDelete = await _db.query(tileStoreTable, where: 'referenceCount <= 0');
    if (toDelete.isEmpty) return 0;
    for (final row in toDelete) {
      final file = File(row['path'] as String);
      if (await file.exists()) {
        try {
          await file.delete();
        } catch (e, s) {
          _log.warning('Failed to delete tile file: ${file.path}', e, s);
        }
      }
    }
    return await _db.delete(tileStoreTable, where: 'referenceCount <= 0');
  }

  Future<StoreStats> getStats() async {
    return StoreStats(
      memory: CacheStats(
        tileCount: _memoryCache.length,
        sizeInBytes: _memoryCache.values.fold(0, (sum, el) => sum + el.length),
      ),
      disk: await _getDiskStats(),
    );
  }

  Future<void> incrementReference(
      String providerId, TileCoordinates coords) async {
    await _db.rawUpdate(
        'UPDATE $tileStoreTable SET referenceCount = referenceCount + 1 WHERE providerId = ? AND z = ? AND x = ? AND y = ?',
        [providerId, coords.z, coords.x, coords.y]);
  }

  Future<void> decrementReference(
      String providerId, TileCoordinates coords) async {
    await _db.rawUpdate(
        'UPDATE $tileStoreTable SET referenceCount = referenceCount - 1 WHERE providerId = ? AND z = ? AND x = ? AND y = ? AND referenceCount > 0',
        [providerId, coords.z, coords.x, coords.y]);
  }

  void _putInMemory(String key, Uint8List bytes) {
    _memoryCache[key] = bytes;
    _enforceMemoryPolicy();
  }

  void _enforceMemoryPolicy() {
    while (_memoryCache.length > _maxMemoryItems) {
      _memoryCache.remove(_memoryCache.keys.first);
    }
  }

  Future<Uint8List?> _getFromDisk(
      String providerId, TileCoordinates coords) async {
    final maps = await _db.query(
      tileStoreTable,
      columns: ['path'],
      where: 'providerId = ? AND z = ? AND x = ? AND y = ?',
      whereArgs: [providerId, coords.z, coords.x, coords.y],
      limit: 1,
    );

    if (maps.isNotEmpty) {
      final path = maps.first['path'] as String;
      final file = File(path);
      try {
        if (await file.exists()) {
          // Asynchronously update lastAccessed without waiting.
          _db.update(
            tileStoreTable,
            {'lastAccessed': DateTime.now().toIso8601String()},
            where: 'providerId = ? AND z = ? AND x = ? AND y = ?',
            whereArgs: [providerId, coords.z, coords.x, coords.y],
          );
          return await file.readAsBytes();
        } else {
          // Self-heal: DB record exists but file is gone.
          _log.warning('Tile file not found at $path, removing stale DB record.');
          await _db.delete(
            tileStoreTable,
            where: 'providerId = ? AND z = ? AND x = ? AND y = ?',
            whereArgs: [providerId, coords.z, coords.x, coords.y],
          );
        }
      } catch (e, s) {
        _log.warning('Failed to read tile file $path or clean up DB record.', e, s);
      }
    }
    return null;
  }

  Future<void> _putOnDisk(
      String providerId, TileCoordinates coords, Uint8List bytes) async {
    final path = await getTilePath(providerId, coords);
    final file = File(path);

    try {
      await file.create(recursive: true);
      await file.writeAsBytes(bytes, flush: true);

      await _db.insert(
          tileStoreTable,
          {
            'providerId': providerId,
            'z': coords.z,
            'x': coords.x,
            'y': coords.y,
            'path': path,
            'sizeInBytes': bytes.length,
            'lastAccessed': DateTime.now().toIso8601String(),
            'referenceCount': 0,
          },
          conflictAlgorithm: ConflictAlgorithm.replace);
    } catch (e, s) {
      _log.warning('Failed to write tile to disk or DB. Path: $path', e, s);
    }
  }

  Future<CacheStats> _getDiskStats() async {
    try {
      final result = await _db.rawQuery(
          'SELECT COUNT(*) as count, SUM(sizeInBytes) as bytes FROM $tileStoreTable');
      return CacheStats(
        tileCount: Sqflite.firstIntValue(result) ?? 0,
        sizeInBytes: (result.first['bytes'] as num?)?.toInt() ?? 0,
      );
    } catch (e, s) {
      _log.severe('Failed to query disk stats.', e, s);
      return const CacheStats();
    }
  }

  /// Lazily initializes and caches the base path for tile files.
  /// Uses the testDirectory if provided, otherwise uses the platform-specific path.
  Future<String> get _tileFilesBaseDir async {
    if (testDirectory != null) return testDirectory!;
    return _tileFilesPath ??= (await getApplicationSupportDirectory()).path;
  }

  @visibleForTesting
  Future<String> getTilePath(String providerId, TileCoordinates coords) async {
    final baseDir = await _tileFilesBaseDir;
    return p.join(baseDir, 'tile_files', providerId, coords.z.toString(),
        coords.x.toString(), '${coords.y}.png');
  }

  String _getTileKey(String providerId, TileCoordinates coords) =>
      '$providerId/${coords.z}/${coords.x}/${coords.y}';
}