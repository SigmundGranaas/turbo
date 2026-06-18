import Foundation
import Observation
import CoreModel
import CoreData

/// A route to follow — solved-route geometry plus the metadata progress needs.
/// `waypoints` carry the original stops so an off-route reroute can re-solve from
/// the live position; they're empty when following a saved track (no solver).
public struct FollowRoute: Equatable, Sendable {
    public let geometry: [LatLng]
    public let distanceM: Double
    public let ascentM: Double
    public let name: String?
    public let waypoints: [LatLng]

    public init(geometry: [LatLng], distanceM: Double, ascentM: Double, name: String? = nil, waypoints: [LatLng] = []) {
        self.geometry = geometry
        self.distanceM = distanceM
        self.ascentM = ascentM
        self.name = name
        self.waypoints = waypoints
    }
}

/// Drives live route-following: streams the user's location, projects it onto the
/// route to compute remaining distance / ETA / progress, flags off-route, and
/// (for planned routes) auto-reroutes from the live position. App-lifetime, like
/// ``RecordingController`` — following survives backgrounding. Mirrors Android's
/// `FollowController`.
@MainActor
@Observable
public final class FollowController {
    public private(set) var isFollowing = false
    public private(set) var geometry: [LatLng] = []
    public private(set) var name: String?
    public private(set) var distanceRemainingM: Double = 0
    public private(set) var etaSeconds: Int?
    public private(set) var fraction: Double = 0
    public private(set) var isOffRoute = false
    public private(set) var arrived = false
    public private(set) var userPosition: LatLng?

    private let location: LocationProvider
    private var route: FollowRoute?
    private var tracker: RouteProgressTracker?
    private var reroute: (@Sendable ([LatLng]) async -> FollowRoute?)?
    private var observation: Task<Void, Never>?
    private var rerouting = false

    public init(location: LocationProvider) { self.location = location }

    /// Begin following `route`. `reroute` (optional) re-solves from a new origin
    /// when the user strays off-route; pass nil for saved-track follow.
    public func start(_ route: FollowRoute, reroute: (@Sendable ([LatLng]) async -> FollowRoute?)? = nil) {
        guard route.geometry.count >= 2 else { return }
        apply(route)
        self.reroute = reroute
        isFollowing = true; arrived = false; isOffRoute = false
        location.requestAlwaysAuthorization()
        location.setBackgroundUpdates(true)
        observation?.cancel()
        observation = Task { [weak self, location] in
            for await fix in location.fixes() { self?.onFix(fix) }
        }
    }

    public func stop() {
        isFollowing = false; route = nil; reroute = nil
        location.setBackgroundUpdates(false)
        observation?.cancel(); observation = nil
        geometry = []; name = nil; distanceRemainingM = 0; etaSeconds = nil
        fraction = 0; isOffRoute = false; arrived = false; userPosition = nil
    }

    private func apply(_ route: FollowRoute) {
        self.route = route
        geometry = route.geometry
        name = route.name
        distanceRemainingM = route.distanceM
        // Fresh arc-length cursor for this route (also reset on reroute).
        tracker = RouteProgressTracker(route: route.geometry, ascentM: route.ascentM)
    }

    private func onFix(_ fix: LocationFix) {
        guard isFollowing, let tracker else { return }
        userPosition = fix.position
        let p = tracker.update(fix.position)
        fraction = p.fraction
        distanceRemainingM = p.distanceRemainingM
        etaSeconds = p.etaSeconds
        isOffRoute = p.offRoute
        arrived = p.arrived
        maybeReroute(from: fix.position)
    }

    private func maybeReroute(from position: LatLng) {
        guard isOffRoute, !rerouting, let reroute, let route, route.waypoints.count >= 2 else { return }
        rerouting = true
        let newWaypoints = [position] + route.waypoints.dropFirst()
        Task { [weak self] in
            let updated = await reroute(newWaypoints)
            if let self, self.isFollowing, let updated {
                self.apply(updated)
                self.isOffRoute = false
            }
            self?.rerouting = false
        }
    }
}
