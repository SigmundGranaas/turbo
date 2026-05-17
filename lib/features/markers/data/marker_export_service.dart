import 'dart:io';
import 'dart:typed_data';

import 'package:file_picker/file_picker.dart';
import 'package:flutter/foundation.dart' show kIsWeb;
import 'package:path_provider/path_provider.dart';
import 'package:share_plus/share_plus.dart';

import '../models/marker.dart';
import 'marker_geojson_serializer.dart';

class MarkerExportService {
  String shareText(Marker marker) {
    final coords =
        '${marker.position.latitude.toStringAsFixed(6)}, '
        '${marker.position.longitude.toStringAsFixed(6)}';
    final buffer = StringBuffer('${marker.title} ($coords)');
    if (marker.description != null && marker.description!.isNotEmpty) {
      buffer.write('\n${marker.description}');
    }
    return buffer.toString();
  }

  Future<void> shareAsText(Marker marker) async {
    await Share.share(shareText(marker));
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
