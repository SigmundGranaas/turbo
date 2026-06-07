import Foundation

/// A named, persisted ``GeoPath`` — a recorded track or saved route, optionally
/// tagged with the activity it represents. Mirrors `domain.SavedPath`.
public struct SavedPath: Identifiable, Hashable, Sendable {
    public let id: String
    public let name: String
    public let path: GeoPath
    public let activityKind: ActivityKindId?

    public init(id: String, name: String, path: GeoPath, activityKind: ActivityKindId? = nil) {
        self.id = id
        self.name = name
        self.path = path
        self.activityKind = activityKind
    }

    /// `recordedAtEpochMs` as a `Date`, when present.
    public var recordedAt: Date? {
        path.recordedAtEpochMs.map { Date(timeIntervalSince1970: Double($0) / 1000) }
    }
}
