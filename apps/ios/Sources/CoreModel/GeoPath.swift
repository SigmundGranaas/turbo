import Foundation

/// Where a ``GeoPath`` came from — drives styling and available verbs.
/// Mirrors `core.geo.GeoPathSource`.
public enum GeoPathSource: String, Sendable, Codable {
    case route, recording, measure, saved, trail, activity
}

/// The single canonical "a line on the map" value type that every feature
/// (routes, recordings, saved tracks, measure tool) converts to and from.
/// Mirrors `core.geo.GeoPath` (trimmed to what the iOS features need so far).
public struct GeoPath: Hashable, Sendable, Codable {
    public let points: [LatLng]
    public let source: GeoPathSource
    public let elevations: [Double]?
    public let distanceM: Double
    public let ascentM: Double?
    public let recordedAtEpochMs: Int64?
    public let movingTimeSeconds: Int?

    public init(
        points: [LatLng],
        source: GeoPathSource,
        elevations: [Double]? = nil,
        distanceM: Double? = nil,
        ascentM: Double? = nil,
        recordedAtEpochMs: Int64? = nil,
        movingTimeSeconds: Int? = nil
    ) {
        self.points = points
        self.source = source
        self.elevations = elevations
        self.distanceM = distanceM ?? GeoMetrics.pathLengthMeters(points)
        self.ascentM = ascentM ?? GeoMetrics.ascentMeters(elevations)
        self.recordedAtEpochMs = recordedAtEpochMs
        self.movingTimeSeconds = movingTimeSeconds
    }

    public var isEmpty: Bool { points.isEmpty }
}

/// Pure geometry helpers. Mirrors `core.geo.GeoMetrics`.
public enum GeoMetrics {
    private static let earthRadiusM = 6_371_000.0

    /// Great-circle distance between two coordinates (Haversine), in metres.
    public static func haversineMeters(_ a: LatLng, _ b: LatLng) -> Double {
        let dLat = (b.lat - a.lat) * .pi / 180
        let dLng = (b.lng - a.lng) * .pi / 180
        let lat1 = a.lat * .pi / 180
        let lat2 = b.lat * .pi / 180
        let h = sin(dLat / 2) * sin(dLat / 2)
            + sin(dLng / 2) * sin(dLng / 2) * cos(lat1) * cos(lat2)
        return 2 * earthRadiusM * asin(min(1, sqrt(h)))
    }

    /// Total length of a polyline in metres.
    public static func pathLengthMeters(_ points: [LatLng]) -> Double {
        guard points.count > 1 else { return 0 }
        return zip(points, points.dropFirst())
            .reduce(0) { $0 + haversineMeters($1.0, $1.1) }
    }

    /// Cumulative positive elevation gain in metres.
    public static func ascentMeters(_ elevations: [Double]?) -> Double? {
        guard let elevations, elevations.count > 1 else { return nil }
        return zip(elevations, elevations.dropFirst())
            .reduce(0) { $0 + max(0, $1.1 - $1.0) }
    }

    /// Cumulative elevation loss in metres (a positive number).
    public static func descentMeters(_ elevations: [Double]?) -> Double? {
        guard let elevations, elevations.count > 1 else { return nil }
        return zip(elevations, elevations.dropFirst())
            .reduce(0) { $0 + max(0, $1.0 - $1.1) }
    }
}
