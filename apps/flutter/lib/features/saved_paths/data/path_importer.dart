import '../models/saved_path.dart';
import 'geojson_parser.dart';
import 'gpx_parser.dart';
import 'kml_parser.dart';

/// Supported import formats. The format is normally inferred from the file
/// extension but content sniffing kicks in when the extension is unknown.
enum ImportFormat { gpx, geoJson, kml }

class PathImportException implements Exception {
  final String message;
  PathImportException(this.message);
  @override
  String toString() => 'PathImportException: $message';
}

/// Top-level entry point for parsing one of the supported track files into
/// a list of [SavedPath] candidates. The repository decides whether to
/// persist them — this function just decodes.
List<SavedPath> importPathContent(String content, {String? filename}) {
  final format = _detect(content, filename);
  switch (format) {
    case ImportFormat.gpx:
      return parseGpx(content);
    case ImportFormat.geoJson:
      return parseGeoJson(content);
    case ImportFormat.kml:
      return parseKml(content);
  }
}

ImportFormat _detect(String content, String? filename) {
  final ext = filename?.toLowerCase().split('.').last;
  switch (ext) {
    case 'gpx':
      return ImportFormat.gpx;
    case 'geojson':
    case 'json':
      return ImportFormat.geoJson;
    case 'kml':
      return ImportFormat.kml;
  }

  // Fall back to content sniffing — useful for "Open with…" payloads where
  // the file may arrive as `track.txt` from a mail client.
  final trimmed = content.trimLeft();
  if (trimmed.startsWith('<?xml') || trimmed.startsWith('<gpx')) {
    return ImportFormat.gpx;
  }
  if (trimmed.startsWith('<kml') || trimmed.startsWith('<?xml')) {
    return ImportFormat.kml;
  }
  if (trimmed.startsWith('{') || trimmed.startsWith('[')) {
    return ImportFormat.geoJson;
  }
  throw PathImportException(
      'Unrecognised file format (filename=$filename). '
      'Supported: .gpx, .geojson/.json, .kml.');
}
