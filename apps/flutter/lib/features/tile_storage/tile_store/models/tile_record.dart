import 'package:flutter/foundation.dart';

/// Represents a raw record from the tile store database.
@immutable
class TileRecord {
  final String providerId;
  final int z;
  final int x;
  final int y;
  final String path;
  final int sizeInBytes;
  final DateTime lastAccessed;
  final int referenceCount;

  const TileRecord({
    required this.providerId,
    required this.z,
    required this.x,
    required this.y,
    required this.path,
    required this.sizeInBytes,
    required this.lastAccessed,
    required this.referenceCount,
  });

  factory TileRecord.fromMap(Map<String, dynamic> map) {
    return TileRecord(
      providerId: map['providerId'] as String,
      z: map['z'] as int,
      x: map['x'] as int,
      y: map['y'] as int,
      path: map['path'] as String,
      sizeInBytes: map['sizeInBytes'] as int,
      lastAccessed: DateTime.parse(map['lastAccessed'] as String),
      referenceCount: map['referenceCount'] as int,
    );
  }
}