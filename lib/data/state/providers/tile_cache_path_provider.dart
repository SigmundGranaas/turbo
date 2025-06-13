import 'package:flutter/foundation.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:path_provider/path_provider.dart';

final cachePathProvider = AutoDisposeFutureProvider<String?>((ref) async {
  if (kIsWeb) return null;
  final directory = await getTemporaryDirectory();
  return directory.path;
});