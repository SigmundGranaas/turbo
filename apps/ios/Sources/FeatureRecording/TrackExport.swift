import Foundation
import CoreModel

/// Track-exchange formats a ``SavedPath`` can be exported to. Mirrors Android's
/// `ExportFormat` (`feature.recording.Gpx`).
public enum ExportFormat: String, CaseIterable, Sendable {
    case gpx, geojson, kml

    public var label: String {
        switch self {
        case .gpx: "GPX"
        case .geojson: "GeoJSON"
        case .kml: "KML"
        }
    }

    public var fileExtension: String { rawValue }

    public var mimeType: String {
        switch self {
        case .gpx: "application/gpx+xml"
        case .geojson: "application/geo+json"
        case .kml: "application/vnd.google-earth.kml+xml"
        }
    }
}

/// Serialises a ``SavedPath`` to GPX 1.1 / GeoJSON / KML — the universal
/// track-exchange formats Garmin, Strava, Komoot, Gaia and Google Earth import.
/// Per-point `<ele>` is emitted when the track captured altitude. Mirrors
/// Android's `feature.recording.Gpx`.
public enum TrackExport {

    public static func serialize(_ path: SavedPath, as format: ExportFormat) -> String {
        switch format {
        case .gpx: gpx(path)
        case .geojson: geoJSON(path)
        case .kml: kml(path)
        }
    }

    /// A safe filename derived from the track name for `format`.
    public static func fileName(for name: String, format: ExportFormat) -> String {
        let trimmed = name.trimmingCharacters(in: .whitespacesAndNewlines)
        let base = trimmed.isEmpty ? "track" : trimmed
        let stem = String(base.replacing(#/[^A-Za-z0-9\-_]+/#, with: "_").prefix(40))
        return "\(stem).\(format.fileExtension)"
    }

    /// Serialise `path` and write it to a uniquely-namespaced temp file, returning
    /// its URL — ready to hand to a `ShareLink` / share sheet.
    public static func writeTemporaryFile(_ path: SavedPath, as format: ExportFormat) throws -> URL {
        let directory = FileManager.default.temporaryDirectory
            .appendingPathComponent("turbo-export-\(UUID().uuidString)", isDirectory: true)
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        let url = directory.appendingPathComponent(fileName(for: path.name, format: format))
        try serialize(path, as: format).write(to: url, atomically: true, encoding: .utf8)
        return url
    }

    // MARK: - Formats

    private static func gpx(_ path: SavedPath) -> String {
        let points = path.path.points
        let elevations = path.path.elevations
        var s = ""
        s += "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n"
        s += "<gpx version=\"1.1\" creator=\"Turbo\" xmlns=\"http://www.topografix.com/GPX/1/1\">\n"
        s += "  <trk>\n"
        s += "    <name>\(escapeXML(path.name))</name>\n"
        s += "    <trkseg>\n"
        for (i, p) in points.enumerated() {
            s += "      <trkpt lat=\"\(p.lat)\" lon=\"\(p.lng)\">"
            if let ele = elevations?[safe: i] { s += "<ele>\(ele)</ele>" }
            s += "</trkpt>\n"
        }
        s += "    </trkseg>\n"
        s += "  </trk>\n"
        s += "</gpx>\n"
        return s
    }

    private static func geoJSON(_ path: SavedPath) -> String {
        let points = path.path.points
        let elevations = path.path.elevations
        let coords = points.enumerated().map { i, p in
            if let ele = elevations?[safe: i] { "[\(p.lng),\(p.lat),\(ele)]" } else { "[\(p.lng),\(p.lat)]" }
        }.joined(separator: ",")
        return "{\"type\":\"Feature\","
            + "\"properties\":{\"name\":\(jsonString(path.name))},"
            + "\"geometry\":{\"type\":\"LineString\",\"coordinates\":[\(coords)]}}"
    }

    private static func kml(_ path: SavedPath) -> String {
        let points = path.path.points
        let elevations = path.path.elevations
        let coords = points.enumerated().map { i, p in
            "\(p.lng),\(p.lat),\(elevations?[safe: i] ?? 0)"
        }.joined(separator: " ")
        var s = ""
        s += "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n"
        s += "<kml xmlns=\"http://www.opengis.net/kml/2.2\">\n  <Document>\n    <Placemark>\n"
        s += "      <name>\(escapeXML(path.name))</name>\n"
        s += "      <LineString><coordinates>\(coords)</coordinates></LineString>\n"
        s += "    </Placemark>\n  </Document>\n</kml>\n"
        return s
    }

    // MARK: - Escaping

    private static func escapeXML(_ s: String) -> String {
        s.replacingOccurrences(of: "&", with: "&amp;")
            .replacingOccurrences(of: "<", with: "&lt;")
            .replacingOccurrences(of: ">", with: "&gt;")
            .replacingOccurrences(of: "\"", with: "&quot;")
    }

    private static func jsonString(_ s: String) -> String {
        "\"" + s.replacingOccurrences(of: "\\", with: "\\\\")
            .replacingOccurrences(of: "\"", with: "\\\"")
            .replacingOccurrences(of: "\n", with: "\\n") + "\""
    }
}

private extension Array {
    subscript(safe index: Int) -> Element? {
        indices.contains(index) ? self[index] : nil
    }
}
