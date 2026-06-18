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

    /// The covered **prefix** of `route` up to `fraction` (0…1) of its length, including the
    /// interpolated split point at the cursor. Empty when ≤ 0; the whole route when ≥ 1. Used to
    /// dim the already-walked portion of a followed guide (US-3). Mirrors Android `routePrefix`.
    public static func routePrefix(_ route: [LatLng], _ fraction: Double) -> [LatLng] {
        guard route.count >= 2, fraction > 0 else { return [] }
        let total = pathLengthMeters(route)
        guard total > 0 else { return [] }
        if fraction >= 1 { return route }
        let target = total * fraction
        var out = [route[0]]
        var cum = 0.0
        for i in 1..<route.count {
            let seg = haversineMeters(route[i - 1], route[i])
            if cum + seg < target {
                out.append(route[i]); cum += seg
            } else {
                let t = seg > 0 ? (target - cum) / seg : 0
                let a = route[i - 1], b = route[i]
                out.append(LatLng(lat: a.lat + (b.lat - a.lat) * t, lng: a.lng + (b.lng - a.lng) * t))
                break
            }
        }
        return out
    }

    /// The remaining **suffix** of `route` from `fraction` onward, meeting `routePrefix` exactly
    /// at the cursor so the guide draws as two segments (covered dim + remaining bright, US-3).
    public static func routeSuffix(_ route: [LatLng], _ fraction: Double) -> [LatLng] {
        guard route.count >= 2, fraction < 1 else { return [] }
        if fraction <= 0 { return route }
        let total = pathLengthMeters(route)
        guard total > 0 else { return route }
        let target = total * fraction
        var out: [LatLng] = []
        var cum = 0.0
        var started = false
        for i in 1..<route.count {
            let seg = haversineMeters(route[i - 1], route[i])
            if !started, cum + seg >= target {
                let t = seg > 0 ? (target - cum) / seg : 0
                let a = route[i - 1], b = route[i]
                out.append(LatLng(lat: a.lat + (b.lat - a.lat) * t, lng: a.lng + (b.lng - a.lng) * t))
                out.append(b)
                started = true
            } else if started {
                out.append(route[i])
            }
            cum += seg
        }
        return out
    }

    // MARK: - Follow-mode progress helpers (the cursor lives in RouteProgress.swift)

    /// Naismith + Langmuir time estimate: 1 h per 5 km flat, plus 1 h per 600 m ascent.
    public static func etaSeconds(distanceM: Double, ascentM: Double?) -> Int {
        let flat = distanceM / 5000 * 3600
        let climb = (ascentM ?? 0) / 600 * 3600
        return Int(flat + climb)
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
