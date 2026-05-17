import 'dart:io';
import 'dart:typed_data';

import 'package:file_picker/file_picker.dart';
import 'package:flutter/foundation.dart' show kIsWeb;
import 'package:flutter/services.dart';
import 'package:path_provider/path_provider.dart';
import 'package:share_plus/share_plus.dart';

import '../../../core/sharing/shareable_link_codec.dart';
import '../models/marker.dart';
import 'marker_geojson_serializer.dart';

class MarkerExportService {
  /// Encodes [marker] into a shareable web URL pointing at [webBaseUrl].
  String buildShareLink(Marker marker, String webBaseUrl) {
    return ShareableLinkCodec.encodeMarker(marker, webBaseUrl);
  }

  /// Copies the share link to the clipboard and opens the system share sheet.
  Future<void> shareAsLink(Marker marker, String webBaseUrl) async {
    final url = buildShareLink(marker, webBaseUrl);
    await Clipboard.setData(ClipboardData(text: url));
    await Share.share(url, subject: marker.title);
  }

  String _buildFilename(Marker marker) {
    final sanitized = marker.title
        .toLowerCase()
        .replaceAll(RegExp(r'[^\w\s-]'), '')
        .trim()
        .replaceAll(RegExp(r'\s+'), '-');

    final now = DateTime.now();
    final date =
        '${now.year}-'
        '${now.month.toString().padLeft(2, '0')}-'
        '${now.day.toString().padLeft(2, '0')}';

    final name = sanitized.isEmpty ? 'marker' : sanitized;
    return '$name-$date.geojson';
  }

  Future<void> shareAsGeoJson(Marker marker) async {
    final content = markerToGeoJson(marker);
    final filename = _buildFilename(marker);
    const mimeType = 'application/geo+json';
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

  Future<String?> saveToFile(Marker marker) async {
    final content = markerToGeoJson(marker);
    final filename = _buildFilename(marker);
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
