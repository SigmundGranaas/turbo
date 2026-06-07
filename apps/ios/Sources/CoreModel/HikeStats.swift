import Foundation

/// Derived, display-ready statistics for a completed hike — distance, duration,
/// ascent, pace and the elevation profile. Pure (computed from a ``GeoPath``) so
/// it's exhaustively unit-testable. Backs the hike-detail screen.
public struct HikeStats: Equatable, Sendable {
    public let distanceMeters: Double
    public let durationSeconds: Int?
    public let ascentMeters: Double?
    public let maxElevationMeters: Double?
    public let elevationProfile: [Double]

    public init(_ path: GeoPath) {
        distanceMeters = path.distanceM
        durationSeconds = path.movingTimeSeconds
        ascentMeters = path.ascentM
        elevationProfile = path.elevations ?? []
        maxElevationMeters = path.elevations?.max()
    }

    /// Average pace in seconds per kilometre, or `nil` without a duration/distance.
    public var averagePaceSecondsPerKm: Int? {
        guard let durationSeconds, distanceMeters > 0 else { return nil }
        return Int(Double(durationSeconds) / (distanceMeters / 1000))
    }

    // MARK: Formatting

    public var formattedDistance: String {
        String(format: "%.1f km", distanceMeters / 1000)
    }

    public var formattedDuration: String? {
        durationSeconds.map(HikeStats.clock)
    }

    public var formattedPace: String? {
        averagePaceSecondsPerKm.map { "\(HikeStats.clock($0)) /km" }
    }

    /// `h:mm:ss` for ≥ 1h, else `mm:ss`.
    private static func clock(_ seconds: Int) -> String {
        let h = seconds / 3600, m = (seconds % 3600) / 60, s = seconds % 60
        return h > 0 ? String(format: "%d:%02d:%02d", h, m, s) : String(format: "%d:%02d", m, s)
    }
}
