import Foundation

/// A named, persisted ``GeoPath`` — a recorded track or saved route, optionally
/// tagged with the activity it represents. Mirrors `domain.SavedPath`.
public struct SavedPath: Identifiable, Hashable, Sendable {
    public let id: String
    public let name: String
    public let path: GeoPath
    public let activityKind: ActivityKindId?
    /// When this track came from following a planned route (D1): the guide it was walked
    /// against, so the saved artifact can redraw it. Nil for plain recordings.
    public let plannedRoute: [LatLng]?
    /// Checkpoint splits recorded while following (D1); empty for plain recordings.
    public let phaseSplits: [PhaseSplit]

    public init(id: String, name: String, path: GeoPath, activityKind: ActivityKindId? = nil,
                plannedRoute: [LatLng]? = nil, phaseSplits: [PhaseSplit] = []) {
        self.id = id
        self.name = name
        self.path = path
        self.activityKind = activityKind
        self.plannedRoute = plannedRoute
        self.phaseSplits = phaseSplits
    }

    /// `recordedAtEpochMs` as a `Date`, when present.
    public var recordedAt: Date? {
        path.recordedAtEpochMs.map { Date(timeIntervalSince1970: Double($0) / 1000) }
    }
}
