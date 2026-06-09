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
    /// How the track's geometry is formed. One tool, three modes (mirrors
    /// Android's CreateTrack), folding the old separate "Measure" in — Line/Draw
    /// with the live distance readout *are* the measure tool.
    public enum Mode: String, CaseIterable, Sendable {
        case route   // tap waypoints; snap to trails via the solver
        case line    // tap waypoints; straight legs
        case draw    // freehand finger stroke
        public var label: String {
            switch self { case .route: "Route"; case .line: "Line"; case .draw: "Draw" }
        }
    }

    public var mode: Mode = .route
    public var preset: RoutePreset = .balanced
    public private(set) var waypoints: [LatLng] = []
    /// Freehand points captured in Draw mode (separate from tapped `waypoints`).
    public private(set) var drawPoints: [LatLng] = []
    /// The polyline to draw — the solver preview/result, or the straight legs.
    public private(set) var geometry: [LatLng] = []
    public private(set) var plan: RoutePlan?
    public private(set) var isSolving = false

    private let routeRepository: RouteRepository
    private let pathRepository: PathRepository
    private var solveTask: Task<Void, Never>?
    /// Snapshots of `waypoints` before each edit, for multi-level undo (capped).
    private var undoStack: [[LatLng]] = []
    private static let undoLimit = 20

    public var canUndo: Bool { !undoStack.isEmpty }

    public init(routeRepository: RouteRepository, pathRepository: PathRepository) {
        self.routeRepository = routeRepository
        self.pathRepository = pathRepository
    }

    public func addWaypoint(_ point: LatLng) {
        guard mode != .draw else { return }   // Draw uses the freehand stroke, not taps
        pushUndo(); waypoints.append(point); resolve()
    }

    /// Begin a fresh freehand stroke (drag start). Replaces any prior stroke.
    public func beginStroke() {
        guard mode == .draw else { return }
        pushUndo()
        drawPoints = []
    }

    /// Append a freehand point (drag move), throttled so we don't store hundreds
    /// of near-duplicate points per second.
    public func appendDrawPoint(_ point: LatLng) {
        guard mode == .draw else { return }
        if let last = drawPoints.last, GeoMetrics.haversineMeters(last, point) < 3 { return }
        drawPoints.append(point)
        resolve()
    }

    public func removeLast() {
        guard !waypoints.isEmpty else { return }
        pushUndo()
        waypoints.removeLast()
        resolve()
    }

    /// Remove a specific waypoint. A route needs ≥2 points, so dropping below that
    /// clears the plan but keeps the remaining points.
    public func removeWaypoint(at index: Int) {
        guard waypoints.indices.contains(index) else { return }
        pushUndo()
        waypoints.remove(at: index)
        resolve()
    }

    /// Reorder a waypoint (the editor's up/down controls).
    public func moveWaypoint(from: Int, to: Int) {
        guard waypoints.indices.contains(from), waypoints.indices.contains(to), from != to else { return }
        pushUndo()
        let point = waypoints.remove(at: from)
        waypoints.insert(point, at: to)
        resolve()
    }

    /// Reposition a waypoint (dragging its pin on the map).
    public func moveWaypoint(at index: Int, to point: LatLng) {
        guard waypoints.indices.contains(index) else { return }
        pushUndo()
        waypoints[index] = point
        resolve()
    }

    /// Insert a stop at the segment where it adds the least detour (Android's
    /// `addStop`). Falls back to appending when there's nothing to insert between.
    public func insertStop(_ point: LatLng) {
        pushUndo()
        waypoints = Self.insertLeastDetour(waypoints, point)
        resolve()
    }

    /// Revert the last edit.
    public func undo() {
        guard let previous = undoStack.popLast() else { return }
        waypoints = previous
        resolve()
    }

    public func clear() {
        solveTask?.cancel(); solveTask = nil
        waypoints = []; drawPoints = []; geometry = []; plan = nil; isSolving = false
        undoStack = []
    }

    private func pushUndo() {
        undoStack.append(waypoints)
        if undoStack.count > Self.undoLimit { undoStack.removeFirst() }
    }

    /// Insert `point` into `waypoints` at the position that minimises added detour.
    static func insertLeastDetour(_ waypoints: [LatLng], _ point: LatLng) -> [LatLng] {
        guard waypoints.count >= 2 else { return waypoints + [point] }
        var bestIndex = waypoints.count
        var bestDelta = Double.greatestFiniteMagnitude
        for i in 0..<(waypoints.count - 1) {
            let a = waypoints[i], b = waypoints[i + 1]
            let delta = GeoMetrics.haversineMeters(a, point) + GeoMetrics.haversineMeters(point, b)
                - GeoMetrics.haversineMeters(a, b)
            if delta < bestDelta { bestDelta = delta; bestIndex = i + 1 }
        }
        var result = waypoints
        result.insert(point, at: bestIndex)
        return result
    }

    public func setMode(_ newMode: Mode) {
        guard newMode != mode else { return }
        // Switching modes starts a fresh track (tapped waypoints and freehand
        // strokes don't mix), matching Android's openTrackTool reset.
        mode = newMode
        waypoints = []; drawPoints = []; geometry = []; plan = nil; undoStack = []
        solveTask?.cancel(); solveTask = nil; isSolving = false
    }
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
        let previousGeometry = plan?.geometry   // keep the last solved line while re-solving
        plan = nil

        if mode == .draw {
            geometry = drawPoints
            isSolving = false
            guard drawPoints.count >= 2 else { return }
            let distance = GeoMetrics.pathLengthMeters(drawPoints)
            plan = RoutePlan(distanceM: distance, durationS: distance / 1.3, ascentM: 0,
                             onTrailPct: 0, surfaces: [:], geometry: drawPoints)
            return
        }

        guard waypoints.count >= 2 else { geometry = waypoints; isSolving = false; return }

        switch mode {
        case .draw:
            break   // handled above
        case .line:
            geometry = waypoints
            let distance = GeoMetrics.pathLengthMeters(waypoints)
            plan = RoutePlan(distanceM: distance, durationS: distance / 1.3, ascentM: 0,
                             onTrailPct: 0, surfaces: [:], geometry: waypoints)
            isSolving = false
        case .route:
            // Hold the previous solved geometry (graceful re-solve) so the line
            // doesn't snap to straight segments mid-edit; fall back to straight.
            geometry = previousGeometry ?? waypoints
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
