import 'package:flutter/foundation.dart';

@immutable
class StoreStats {
  final CacheStats memory;
  final CacheStats disk;

  const StoreStats({required this.memory, required this.disk});
}

@immutable
class CacheStats {
  final int tileCount;
  final int sizeInBytes;

  const CacheStats({
    this.tileCount = 0,
    this.sizeInBytes = 0,
  });
}