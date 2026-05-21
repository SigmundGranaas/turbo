import 'dart:io';

import 'package:flutter/foundation.dart';
import 'package:path/path.dart' as p;
import 'package:path_provider/path_provider.dart';

/// Stores marker photo bytes on disk under `<documents>/marker_photos/`.
///
/// The returned [filePath] is the absolute path on the device; persistence
/// metadata (which marker owns which file) is the [MarkerPhotoDataStore]'s
/// job. On web the service is a no-op shim — the markers feature surfaces
/// "Photos require a mobile or desktop device" before invoking it.
class PhotoStorageService {
  /// Copies [source] into the app's documents dir under a deterministic name
  /// derived from [filename]. Returns the destination path.
  Future<String> savePhoto(File source, {required String filename}) async {
    if (kIsWeb) {
      throw UnsupportedError(
          'PhotoStorageService.savePhoto is not supported on web.');
    }
    final dir = await _photosDir();
    final dest = File(p.join(dir.path, filename));
    if (await dest.exists()) {
      await dest.delete();
    }
    await source.copy(dest.path);
    return dest.path;
  }

  /// Deletes the photo file at [filePath]. Silently no-ops if the file is
  /// already gone (idempotent — safe to call from cascading cleanup paths).
  Future<void> deletePhoto(String filePath) async {
    if (kIsWeb) return;
    try {
      final file = File(filePath);
      if (await file.exists()) {
        await file.delete();
      }
    } catch (_) {
      // Swallow — failure to clean up a file shouldn't block the user from
      // removing a photo from the UI.
    }
  }

  Future<Directory> _photosDir() async {
    final docs = await getApplicationDocumentsDirectory();
    final dir = Directory(p.join(docs.path, 'marker_photos'));
    if (!dir.existsSync()) dir.createSync(recursive: true);
    return dir;
  }
}
