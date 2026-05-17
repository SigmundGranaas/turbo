import 'dart:io';

import 'package:file_picker/file_picker.dart';
import 'package:flutter/foundation.dart' show kIsWeb;
import 'package:flutter/services.dart';
import 'package:path_provider/path_provider.dart';
import 'package:share_plus/share_plus.dart';

import '../../../core/sharing/shareable_link_codec.dart';
import '../models/saved_path.dart';
import 'geojson_serializer.dart';
import 'gpx_serializer.dart';

enum ExportFormat { gpx, geoJson }

class PathExportService {
  /// Encodes [path] into a shareable web URL pointing at [webBaseUrl].
  /// Throws [LinkTooLargeException] for paths whose encoded URL exceeds the
  /// safe URL-length budget — callers should fall back to GPX sharing.
  String buildShareLink(SavedPath path, String webBaseUrl) {
    return ShareableLinkCodec.encodePath(path, webBaseUrl);
  }

  /// Copies the share link to the clipboard and opens the system share sheet.
  Future<void> shareAsLink(SavedPath path, String webBaseUrl) async {
    final url = buildShareLink(path, webBaseUrl);
    await Clipboard.setData(ClipboardData(text: url));
    await Share.share(url, subject: path.title);
  }

  String serialize(SavedPath path, ExportFormat format) {
    return switch (format) {
      ExportFormat.gpx => savedPathToGpx(path),
      ExportFormat.geoJson => savedPathToGeoJson(path),
    };
  }

  String buildFilename(SavedPath path, ExportFormat format) {
    final extension = switch (format) {
      ExportFormat.gpx => 'gpx',
      ExportFormat.geoJson => 'geojson',
    };

    final sanitized = path.title
        .toLowerCase()
        .replaceAll(RegExp(r'[^\w\s-]'), '')
        .trim()
        .replaceAll(RegExp(r'\s+'), '-');

    final date =
        '${path.createdAt.year}-'
        '${path.createdAt.month.toString().padLeft(2, '0')}-'
        '${path.createdAt.day.toString().padLeft(2, '0')}';

    final name = sanitized.isEmpty ? 'path' : sanitized;

    return '$name-$date.$extension';
  }

  Future<void> share(SavedPath path, ExportFormat format) async {
    final content = serialize(path, format);
    final filename = buildFilename(path, format);
    final mimeType = switch (format) {
      ExportFormat.gpx => 'application/gpx+xml',
      ExportFormat.geoJson => 'application/geo+json',
    };
    final bytes = Uint8List.fromList(content.codeUnits);

    if (kIsWeb) {
      final xFile = XFile.fromData(bytes, name: filename, mimeType: mimeType);
      await Share.shareXFiles([xFile]);
    } else {
      final tempDir = await getTemporaryDirectory();
      final file = File('${tempDir.path}/$filename');
      await file.writeAsString(content);
      final xFile = XFile(file.path, mimeType: mimeType);
      await Share.shareXFiles([xFile]);
    }
  }

  Future<String?> saveToFile(SavedPath path, ExportFormat format) async {
    final content = serialize(path, format);
    final filename = buildFilename(path, format);
    final bytes = Uint8List.fromList(content.codeUnits);

    final result = await FilePicker.platform.saveFile(
      dialogTitle: filename,
      fileName: filename,
      bytes: bytes,
    );

    if (result != null && !kIsWeb) {
      await File(result).writeAsString(content);
    }

    return result;
  }
}
