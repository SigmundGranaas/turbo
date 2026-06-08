import Foundation
import Observation
import CoreModel
import CoreData

/// Drives route building: collect waypoints, solve (snap-to-trail via the
/// pathfinder, or straight legs), preview the polyline, and save the result as a
/// path. Mirrors `feature.map.RouteViewModel` (Android).
@MainActor
@Observable
public final class RouteViewModel {
    /// How legs between waypoints are formed.
    public enum Mode: String, CaseIterable, Sendable {
        case route   // snap to trails via the solver
        case line    // straight legs
        public var label: String { self == .route ? "Route" : "Line" }
    }

    public var mode: Mode = .route
    public var preset: RoutePreset = .balanced
    public private(set) var waypoints: [LatLng] = []
    /// The polyline to draw — the solver preview/result, or the straight legs.
    public private(set) var geometry: [LatLng] = []
    public private(set) var plan: RoutePlan?
    public private(set) var isSolving = false

    private let routeRepository: RouteRepository
    private let pathRepository: PathRepository
    private var solveTask: Task<Void, Never>?

    public init(routeRepository: RouteRepository, pathRepository: PathRepository) {
        self.routeRepository = routeRepository
        self.pathRepository = pathRepository
    }

    public func addWaypoint(_ point: LatLng) { waypoints.append(point); resolve() }

    public func removeLast() {
        guard !waypoints.isEmpty else { return }
        waypoints.removeLast()
        resolve()
    }

    public func clear() {
        solveTask?.cancel(); solveTask = nil
        waypoints = []; geometry = []; plan = nil; isSolving = false
    }

    public func setMode(_ newMode: Mode) { mode = newMode; resolve() }
    public func setPreset(_ newPreset: RoutePreset) { preset = newPreset; if mode == .route { resolve() } }

    /// Persist the solved route as a recorded path.
    public func saveAsPath(name: String) {
        guard let plan, !plan.geometry.isEmpty else { return }
        let trimmed = name.trimmingCharacters(in: .whitespacesAndNewlines)
        let path = SavedPath(
            id: "route-\(UUID().uuidString)",
            name: trimmed.isEmpty ? "Route" : trimmed,
            path: GeoPath(points: plan.geometry, source: .route, distanceM: plan.distanceM, ascentM: plan.ascentM),
            activityKind: .hiking
        )
        Task { [pathRepository] in await pathRepository.upsert(path) }
    }

    // MARK: - Solve

    private func resolve() {
        solveTask?.cancel(); solveTask = nil
        plan = nil
        guard waypoints.count >= 2 else { geometry = waypoints; isSolving = false; return }

        switch mode {
        case .line:
            geometry = waypoints
            let distance = GeoMetrics.pathLengthMeters(waypoints)
            plan = RoutePlan(distanceM: distance, durationS: distance / 1.3, ascentM: 0,
                             onTrailPct: 0, surfaces: [:], geometry: waypoints)
            isSolving = false
        case .route:
            geometry = waypoints   // show straight preview until the solver streams
            isSolving = true
            let stream = routeRepository.planStream(points: waypoints, preset: preset, profile: "foot")
            solveTask = Task { [weak self] in
                for await event in stream {
                    switch event {
                    case .progress(let coords): self?.geometry = coords
                    case .result(let plan): self?.plan = plan; self?.geometry = plan.geometry; self?.isSolving = false
                    case .failure: self?.isSolving = false
                    }
                }
                self?.isSolving = false
            }
        }
    }
}
