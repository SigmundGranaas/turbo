import '../models/saved_path.dart';

/// Serializes a [SavedPath] to GPX 1.1 XML format.
String savedPathToGpx(SavedPath path) {
  final buffer = StringBuffer();

  buffer.writeln('<?xml version="1.0" encoding="UTF-8"?>');
  buffer.writeln(
    '<gpx version="1.1" creator="Turbo"'
    ' xmlns="http://www.topografix.com/GPX/1/1"'
    ' xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"'
    ' xsi:schemaLocation="http://www.topografix.com/GPX/1/1'
    ' http://www.topografix.com/GPX/1/1/gpx.xsd">',
  );

  // Metadata
  buffer.writeln('  <metadata>');
  buffer.writeln('    <name>${_escapeXml(path.title)}</name>');
  if (path.description != null && path.description!.isNotEmpty) {
    buffer.writeln('    <desc>${_escapeXml(path.description!)}</desc>');
  }
  buffer.writeln('    <time>${path.createdAt.toUtc().toIso8601String()}</time>');
  buffer.writeln('  </metadata>');

  // Track
  buffer.writeln('  <trk>');
  buffer.writeln('    <name>${_escapeXml(path.title)}</name>');
  buffer.writeln('    <trkseg>');
  for (final point in path.points) {
    buffer.writeln(
      '      <trkpt lat="${point.latitude}" lon="${point.longitude}"/>',
    );
  }
  buffer.writeln('    </trkseg>');
  buffer.writeln('  </trk>');

  buffer.writeln('</gpx>');

  return buffer.toString();
}

String _escapeXml(String input) {
  return input
      .replaceAll('&', '&amp;')
      .replaceAll('<', '&lt;')
      .replaceAll('>', '&gt;')
      .replaceAll('"', '&quot;')
      .replaceAll("'", '&apos;');
}
