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
    /// Explicit checkpoints (US-3 / D3): the route stops merged with saved markers near the route,
    /// ordered by arc-length. When empty, checkpoints fall back to the stops in `waypoints` named
    /// B, C, … — so saved-track follows and reroutes keep working unchanged.
    public let phasePositions: [LatLng]
    public let phaseNames: [String]

    public init(geometry: [LatLng], distanceM: Double, ascentM: Double, name: String? = nil,
                waypoints: [LatLng] = [], phasePositions: [LatLng] = [], phaseNames: [String] = []) {
        self.geometry = geometry
        self.distanceM = distanceM
        self.ascentM = ascentM
        self.name = name
        self.waypoints = waypoints
        self.phasePositions = phasePositions
        self.phaseNames = phaseNames
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
    /// The real travelled polyline captured while following (US-3 — drawn over the guide).
    public private(set) var capturedPoints: [LatLng] = []
    /// Distance actually travelled so far (m) — the captured track, not the planned route.
    public private(set) var capturedDistanceM: Double = 0
    /// Accumulated climb / drop (m) over the travelled track (Follow = Record).
    public private(set) var capturedAscentM: Double = 0
    public private(set) var capturedDescentM: Double = 0
    /// Moving time since the follow started (s).
    public private(set) var elapsedSeconds: Int = 0
    /// Checkpoints crossed so far with split times (US-3).
    public private(set) var phaseSplits: [PhaseSplit] = []
    /// The next checkpoint's name + distance to it, or nil when all are crossed.
    public private(set) var nextPhaseName: String?
    public private(set) var nextPhaseDistanceM: Double?

    /// On-map checkpoints with their crossed state (US-3): the route's stops (plus any saved
    /// markers near the route, D3), crossed up to however many splits have been recorded.
    public var phaseMarkers: [(position: LatLng, crossed: Bool)] {
        phasePositions.enumerated().map { ($0.element, $0.offset < phaseSplits.count) }
    }

    private let location: LocationProvider
    private let pathRepository: PathRepository
    private var route: FollowRoute?
    private var tracker: RouteProgressTracker?
    private var reroute: (@Sendable ([LatLng]) async -> FollowRoute?)?
    private var observation: Task<Void, Never>?
    private var ticker: Task<Void, Never>?
    private var rerouting = false
    /// The real travelled track captured while following (Follow = Record).
    private var capture = CapturedTrack()
    private var startedAt: Date?
    // Phase (checkpoint) state (US-3).
    private var phasePositions: [LatLng] = []
    private var phaseNames: [String] = []
    private var lastPhaseDistanceM: Double = 0
    private var lastPhaseSec = 0

    public init(location: LocationProvider, pathRepository: PathRepository) {
        self.location = location
        self.pathRepository = pathRepository
    }

    /// Begin following `route`. `reroute` (optional) re-solves from a new origin
    /// when the user strays off-route; pass nil for saved-track follow.
    public func start(_ route: FollowRoute, reroute: (@Sendable ([LatLng]) async -> FollowRoute?)? = nil) {
        guard route.geometry.count >= 2 else { return }
        apply(route)
        self.reroute = reroute
        isFollowing = true; arrived = false; isOffRoute = false
        // Fresh capture for this follow (Follow = Record).
        capture = CapturedTrack(); capturedDistanceM = 0; capturedAscentM = 0; capturedDescentM = 0; capturedPoints = []
        phaseSplits = []; nextPhaseName = nil; nextPhaseDistanceM = nil; lastPhaseDistanceM = 0; lastPhaseSec = 0
        elapsedSeconds = 0; startedAt = Date()
        location.requestAlwaysAuthorization()
        location.setBackgroundUpdates(true)
        observation?.cancel()
        observation = Task { [weak self, location] in
            for await fix in location.fixes() { self?.onFix(fix) }
        }
        ticker?.cancel()
        ticker = Task { [weak self] in
            while !Task.isCancelled {
                try? await Task.sleep(for: .seconds(1))
                guard let self, self.isFollowing, let started = self.startedAt else { continue }
                self.elapsedSeconds = Int(Date().timeIntervalSince(started))
            }
        }
    }

    /// Stop following and AUTO-SAVE the travelled track (D1) — unless it's too short to be
    /// worth keeping. What we persist is the real line walked, identical to a recording of
    /// the same fixes; the planned route is untouched.
    public func stop() {
        isFollowing = false
        location.setBackgroundUpdates(false)
        observation?.cancel(); observation = nil
        ticker?.cancel(); ticker = nil
        autoSave()
        route = nil; reroute = nil
        geometry = []; name = nil; distanceRemainingM = 0; etaSeconds = nil
        fraction = 0; isOffRoute = false; arrived = false; userPosition = nil
        capture = CapturedTrack(); capturedDistanceM = 0; capturedAscentM = 0; capturedDescentM = 0; capturedPoints = []
        phaseSplits = []; nextPhaseName = nil; nextPhaseDistanceM = nil; lastPhaseDistanceM = 0; lastPhaseSec = 0
        elapsedSeconds = 0; startedAt = nil
    }

    private func autoSave() {
        guard capture.points.count >= 2, capture.distanceM >= Self.minSaveM else { return }
        let elevations = capture.elevations.isEmpty ? nil : capture.elevations
        let saved = SavedPath(
            id: "follow-\(UUID().uuidString)",
            name: name.map { "\($0) (followed)" } ?? "Followed route \(Int(capture.distanceM)) m",
            path: GeoPath(
                points: capture.points,
                source: .recording,
                elevations: elevations,
                recordedAtEpochMs: startedAt.map { Int64($0.timeIntervalSince1970 * 1000) },
                movingTimeSeconds: elapsedSeconds
            ),
            activityKind: .hiking
        )
        let repository = pathRepository
        Task { await repository.upsert(saved) }
    }

    /// Skip auto-saving trivially short follows (you barely moved).
    private static let minSaveM = 50.0

    private func apply(_ route: FollowRoute) {
        self.route = route
        geometry = route.geometry
        name = route.name
        distanceRemainingM = route.distanceM
        // Checkpoints (US-3): explicit positions+names when supplied (stops + nearby saved markers,
        // arc-length ordered, D3); otherwise the stops after the origin, lettered B, C, ….
        if !route.phasePositions.isEmpty {
            phasePositions = route.phasePositions
            phaseNames = route.phaseNames
        } else {
            let stops = Array(route.waypoints.dropFirst())
            phasePositions = stops
            phaseNames = stops.indices.map { String(UnicodeScalar(UInt8(65 + 1 + $0))) } // B, C, …
        }
        // Fresh arc-length cursor for this route (also reset on reroute).
        tracker = RouteProgressTracker(route: route.geometry, ascentM: route.ascentM, phasePositions: phasePositions)
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
        // Capture the real travelled track with the SAME engine recording uses, so the
        // saved follow is identical to a recording of the same fixes (Follow = Record).
        capture = TrackCapture.append(capture, fix)
        capturedPoints = capture.points
        capturedDistanceM = capture.distanceM
        capturedAscentM = capture.ascentM
        capturedDescentM = capture.descentM
        // Record a split for each checkpoint the cursor just passed (US-3).
        if tracker.passedPhaseCount > phaseSplits.count {
            for i in phaseSplits.count..<tracker.passedPhaseCount {
                phaseSplits.append(PhaseSplit(
                    index: i,
                    name: i < phaseNames.count ? phaseNames[i] : "Checkpoint \(i + 1)",
                    crossedAtEpochMs: Int64(Date().timeIntervalSince1970 * 1000),
                    splitDistanceM: capturedDistanceM - lastPhaseDistanceM,
                    splitSeconds: elapsedSeconds - lastPhaseSec
                ))
                lastPhaseDistanceM = capturedDistanceM
                lastPhaseSec = elapsedSeconds
            }
        }
        let nextIdx = tracker.nextPhaseIndex
        nextPhaseName = nextIdx < phaseNames.count ? phaseNames[nextIdx] : nil
        nextPhaseDistanceM = nextPhaseName != nil ? tracker.distanceToPhase(nextIdx) : nil
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
