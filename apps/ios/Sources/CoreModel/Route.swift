import Foundation

/// A solved route from the Turbo pathfinder. Geometry is ordered WGS84 points.
/// Mirrors `domain.RoutePlan`.
public struct RoutePlan: Equatable, Sendable {
    public let distanceM: Double
    public let durationS: Double
    public let ascentM: Double
    public let onTrailPct: Double
    public let surfaces: [String: Double]
    public let geometry: [LatLng]

    public init(distanceM: Double, durationS: Double, ascentM: Double,
                onTrailPct: Double, surfaces: [String: Double], geometry: [LatLng]) {
        self.distanceM = distanceM
        self.durationS = durationS
        self.ascentM = ascentM
        self.onTrailPct = onTrailPct
        self.surfaces = surfaces
        self.geometry = geometry
    }
}

/// Trip-style presets the pathfinder accepts. Mirrors `domain.RoutePreset`.
public enum RoutePreset: String, CaseIterable, Sendable {
    case balanced, avoidRoads, direct, easyGrade, trailPurist

    /// The `preset` key the API expects.
    public var key: String {
        switch self {
        case .balanced: "balanced"
        case .avoidRoads: "avoid_roads"
        case .direct: "direct"
        case .easyGrade: "easy_grade"
        case .trailPurist: "trail_purist"
        }
    }

    public var label: String {
        switch self {
        case .balanced: "Balanced"
        case .avoidRoads: "Avoid roads"
        case .direct: "Direct"
        case .easyGrade: "Easy grade"
        case .trailPurist: "Trail purist"
        }
    }
}

/// Streamed solver events: live best-path snapshots, then a terminal result/failure.
/// Mirrors `domain.RouteStreamEvent`.
public enum RouteStreamEvent: Equatable, Sendable {
    /// A best-path-so-far snapshot; the latest replaces the previous preview.
    case progress([LatLng])
    case result(RoutePlan)
    case failure(String)
}
