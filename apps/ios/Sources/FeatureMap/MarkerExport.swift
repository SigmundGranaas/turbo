import Foundation
import CoreModel

/// Serialises a single ``Marker`` to a GPX 1.1 **waypoint** — so a saved spot can
/// be shared into Garmin / Gaia / etc., the marker analogue of the track export.
public enum MarkerExport {

    public static func gpx(_ marker: Marker) -> String {
        var s = ""
        s += "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n"
        s += "<gpx version=\"1.1\" creator=\"Turbo\" xmlns=\"http://www.topografix.com/GPX/1/1\">\n"
        s += "  <wpt lat=\"\(marker.position.lat)\" lon=\"\(marker.position.lng)\">\n"
        s += "    <name>\(escapeXML(marker.name))</name>\n"
        if let notes = marker.notes, !notes.isEmpty {
            s += "    <desc>\(escapeXML(notes))</desc>\n"
        }
        s += "  </wpt>\n"
        s += "</gpx>\n"
        return s
    }

    /// Write the waypoint GPX to a uniquely-namespaced temp file for sharing.
    public static func writeTemporaryFile(_ marker: Marker) throws -> URL {
        let directory = FileManager.default.temporaryDirectory
            .appendingPathComponent("turbo-marker-\(UUID().uuidString)", isDirectory: true)
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        let stem = marker.name.trimmingCharacters(in: .whitespacesAndNewlines)
            .replacing(#/[^A-Za-z0-9\-_]+/#, with: "_")
        let url = directory.appendingPathComponent("\(stem.isEmpty ? "marker" : stem).gpx")
        try gpx(marker).write(to: url, atomically: true, encoding: .utf8)
        return url
    }

    private static func escapeXML(_ s: String) -> String {
        s.replacingOccurrences(of: "&", with: "&amp;")
            .replacingOccurrences(of: "<", with: "&lt;")
            .replacingOccurrences(of: ">", with: "&gt;")
            .replacingOccurrences(of: "\"", with: "&quot;")
    }
}
