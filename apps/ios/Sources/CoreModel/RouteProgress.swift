import Foundation

/// A snapshot of how far a position is along a planned route. Replaces the old
/// global-nearest `GeoMetrics.progress`, which broke on loops / out-and-back.
public struct RouteProgress: Equatable, Sendable {
    /// 0…1 along the route (monotonic — only climbs with forward progress).
    public let fraction: Double
    /// Metres still to run, measured *along the route*.
    public let distanceRemainingM: Double
    public let etaSeconds: Int?
    public let offRoute: Bool
    public let arrived: Bool

    public init(fraction: Double, distanceRemainingM: Double, etaSeconds: Int?, offRoute: Bool, arrived: Bool) {
        self.fraction = fraction
        self.distanceRemainingM = distanceRemainingM
        self.etaSeconds = etaSeconds
        self.offRoute = offRoute
        self.arrived = arrived
    }
}

/// Tracks a position along a planned route using a **monotonic arc-length
/// cursor**. Each fix is matched to the nearest route point *within a forward
/// window* around the cursor — not the global nearest — so returning to a start
/// that coincides with the end reads as ~100 % / arrived, never as 0 % /
/// moving-away. Stateful; feed fixes in order. Mirrors the Android
/// `RouteProgressTracker`; both are pinned by `fixtures/tracking/progress/*`.
public final class RouteProgressTracker {
    private let route: [LatLng]
    private let cumulative: [Double]   // arc-length at each vertex
    private let total: Double
    private let ascentM: Double?
    private let windowBackM: Double
    private let windowAheadM: Double
    private let offRouteM: Double
    private let arriveEndM: Double
    private let offRouteStreakNeeded: Int

    private var cursor: Double = 0
    private var offRouteStreak = 0

    public var fraction: Double { total > 0 ? min(max(cursor / total, 0), 1) : 0 }

    public init(
        route: [LatLng],
        ascentM: Double? = nil,
        windowBackM: Double = 60,
        windowAheadM: Double = 400,
        offRouteM: Double = 50,
        arriveEndM: Double = 30,
        offRouteStreakNeeded: Int = 3
    ) {
        self.route = route
        self.ascentM = ascentM
        self.windowBackM = windowBackM
        self.windowAheadM = windowAheadM
        self.offRouteM = offRouteM
        self.arriveEndM = arriveEndM
        self.offRouteStreakNeeded = offRouteStreakNeeded

        var cum: [Double] = [0]
        cum.reserveCapacity(route.count)
        for (a, b) in zip(route, route.dropFirst()) {
            cum.append(cum.last! + GeoMetrics.haversineMeters(a, b))
        }
        self.cumulative = cum
        self.total = cum.last ?? 0
    }

    /// Advance the cursor with a new position and return the resulting progress.
    @discardableResult
    public func update(_ position: LatLng) -> RouteProgress {
        guard route.count >= 2, total > 0 else {
            return RouteProgress(fraction: 0, distanceRemainingM: total, etaSeconds: nil, offRoute: false, arrived: false)
        }

        let lo = cursor - windowBackM
        let hi = cursor + windowAheadM
        var bestDist = Double.greatestFiniteMagnitude
        var bestS = cursor

        for i in 0..<(route.count - 1) {
            let segStart = cumulative[i], segEnd = cumulative[i + 1]
            if segEnd < lo || segStart > hi { continue }   // segment entirely outside the window
            let (proj, t) = GeoMetrics.projectFraction(route[i], route[i + 1], position)
            let sHere = segStart + (segEnd - segStart) * t
            guard sHere >= lo && sHere <= hi else { continue }   // projection lands outside the window
            let d = GeoMetrics.haversineMeters(proj, position)
            if d < bestDist { bestDist = d; bestS = sHere }
        }

        cursor = max(cursor, bestS)   // monotonic; small backtracks don't yo-yo the number

        let onRoute = bestDist <= offRouteM
        offRouteStreak = onRoute ? 0 : offRouteStreak + 1
        let offRoute = offRouteStreak >= offRouteStreakNeeded

        let remaining = max(0, total - cursor)
        let frac = fraction
        let eta = GeoMetrics.etaSeconds(distanceM: remaining, ascentM: ascentM.map { $0 * (1 - frac) })
        // Arrived only once the whole arc is traversed AND we're physically at the end.
        let atEnd = cursor >= total - arriveEndM
            && GeoMetrics.haversineMeters(position, route[route.count - 1]) <= arriveEndM

        return RouteProgress(fraction: frac, distanceRemainingM: remaining, etaSeconds: eta, offRoute: offRoute, arrived: atEnd)
    }
}
