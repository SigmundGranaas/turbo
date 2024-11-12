import 'package:flutter/foundation.dart';
import 'package:path_provider/path_provider.dart';
import 'package:riverpod_annotation/riverpod_annotation.dart';

part 'tile_cache_path_provider.g.dart';

@riverpod
Future<String?> cachePath(CachePathRef ref) async {
  if (kIsWeb) return null;
  final directory = await getTemporaryDirectory();
  return directory.path;
}