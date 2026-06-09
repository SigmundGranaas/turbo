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

    // MARK: - Follow-mode progress (mirrors Android GeoMetrics.progress)

    /// Naismith + Langmuir time estimate: 1 h per 5 km flat, plus 1 h per 600 m ascent.
    public static func etaSeconds(distanceM: Double, ascentM: Double?) -> Int {
        let flat = distanceM / 5000 * 3600
        let climb = (ascentM ?? 0) / 600 * 3600
        return Int(flat + climb)
    }

    /// Shortest distance from `position` to the route polyline, in metres.
    public static func distanceToPath(_ points: [LatLng], _ position: LatLng) -> Double {
        guard points.count > 1 else {
            return points.first.map { haversineMeters($0, position) } ?? .greatestFiniteMagnitude
        }
        return zip(points, points.dropFirst()).reduce(Double.greatestFiniteMagnitude) { best, seg in
            min(best, haversineMeters(projectFraction(seg.0, seg.1, position).point, position))
        }
    }

    /// Where `position` sits along the route: completed fraction, distance still to
    /// run, and an ETA for the remainder (ascent scaled by what's left).
    public static func progress(_ points: [LatLng], position: LatLng, ascentM: Double? = nil) -> JourneyProgress? {
        let total = pathLengthMeters(points)
        guard total > 0 else { return nil }
        var bestDist = Double.greatestFiniteMagnitude
        var bestAlong = 0.0
        var cumulative = 0.0
        for (a, b) in zip(points, points.dropFirst()) {
            let seg = haversineMeters(a, b)
            let (proj, t) = projectFraction(a, b, position)
            let d = haversineMeters(proj, position)
            if d < bestDist { bestDist = d; bestAlong = cumulative + seg * t }
            cumulative += seg
        }
        let fraction = min(max(bestAlong / total, 0), 1)
        let remaining = total - bestAlong
        let eta = etaSeconds(distanceM: remaining, ascentM: ascentM.map { $0 * (1 - fraction) })
        return JourneyProgress(fraction: fraction, distanceRemainingM: remaining, etaSeconds: eta)
    }

    /// Project `p` onto segment `a→b` (planar — fine at trail scale). Returns the
    /// projected point and the clamped fraction `t` along the segment.
    static func projectFraction(_ a: LatLng, _ b: LatLng, _ p: LatLng) -> (point: LatLng, t: Double) {
        let abx = b.lng - a.lng, aby = b.lat - a.lat
        let apx = p.lng - a.lng, apy = p.lat - a.lat
        let denom = abx * abx + aby * aby
        let t = denom > 0 ? min(max((apx * abx + apy * aby) / denom, 0), 1) : 0
        return (LatLng(lat: a.lat + t * aby, lng: a.lng + t * abx), t)
    }
}

/// Where the user is along a followed route. Mirrors Android `JourneyProgress`.
public struct JourneyProgress: Equatable, Sendable {
    public let fraction: Double
    public let distanceRemainingM: Double
    public let etaSeconds: Int?
    public init(fraction: Double, distanceRemainingM: Double, etaSeconds: Int?) {
        self.fraction = fraction
        self.distanceRemainingM = distanceRemainingM
        self.etaSeconds = etaSeconds
    }
}
