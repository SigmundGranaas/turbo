import 'dart:io';

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
    await SharePlus.instance.share(ShareParams(uri: Uri.parse(url), subject: marker.title));
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
    await _shareGeoJsonContent(
      content: markerToGeoJson(marker),
      filename: _buildFilename(marker),
    );
  }

  /// Bulk variant: shares a single FeatureCollection holding every marker.
  /// Filename is derived from the first marker's title plus the count.
  Future<void> shareManyAsGeoJson(List<Marker> markers) async {
    if (markers.isEmpty) return;
    final content = markersToGeoJson(markers);
    final base = _buildFilename(markers.first).replaceAll('.geojson', '');
    final filename = '$base-${markers.length}.geojson';
    await _shareGeoJsonContent(content: content, filename: filename);
  }

  Future<void> _shareGeoJsonContent({
    required String content,
    required String filename,
  }) async {
    const mimeType = 'application/geo+json';
    final bytes = Uint8List.fromList(content.codeUnits);
    if (kIsWeb) {
      final xFile = XFile.fromData(bytes, name: filename, mimeType: mimeType);
      await SharePlus.instance.share(ShareParams(files: [xFile]));
    } else {
      final tempDir = await getTemporaryDirectory();
      final file = File('${tempDir.path}/$filename');
      await file.writeAsString(content);
      final xFile = XFile(file.path, mimeType: mimeType);
      await SharePlus.instance.share(ShareParams(files: [xFile]));
    }
  }

  Future<String?> saveToFile(Marker marker) async {
    final content = markerToGeoJson(marker);
    final filename = _buildFilename(marker);
    final bytes = Uint8List.fromList(content.codeUnits);

    final result = await FilePicker.saveFile(
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
