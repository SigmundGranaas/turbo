import Foundation

/// Geo + unit formatting shared across screens. Mirrors `core.geo` helpers
/// (`formatCoords`) plus the byte/size formatting the offline screen needs.
public enum Geo {

    /// `"69.6412° N, 20.1003° E"` — shared by the marker sheet and detail host.
    /// Mirrors `core.geo.formatCoords`.
    public static func formatCoords(_ p: LatLng) -> String {
        let ns = p.lat >= 0 ? "N" : "S"
        let ew = p.lng >= 0 ? "E" : "W"
        return String(format: "%.4f° %@, %.4f° %@", abs(p.lat), ns, abs(p.lng), ew)
    }

    /// Human-readable download size, e.g. `"640 MB"`, `"1.4 GB"`.
    public static func formatBytes(_ bytes: Int64) -> String {
        let formatter = ByteCountFormatter()
        formatter.countStyle = .file
        formatter.allowedUnits = [.useMB, .useGB]
        return formatter.string(fromByteCount: bytes)
    }

    /// The geographic centre of a bounding box.
    public static func center(of bounds: GeoBounds) -> LatLng {
        LatLng(
            lat: (bounds.south + bounds.north) / 2,
            lng: (bounds.west + bounds.east) / 2
        )
    }
}
