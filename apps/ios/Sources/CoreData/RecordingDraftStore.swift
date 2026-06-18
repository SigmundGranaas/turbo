import Foundation
import CoreModel

/// A recording snapshot persisted across process death so a track survives an app kill.
/// Mirrors Android's `RecordingDraft`.
public struct RecordingDraft: Sendable, Equatable {
    public let points: [LatLng]
    public let elevations: [Double]
    public let elapsedSeconds: Int
    /// When true the session was paused; `bufferPoints`/`bufferElevations` hold the walk
    /// captured while paused so a kill-while-paused doesn't lose it (US-4).
    public let paused: Bool
    public let bufferPoints: [LatLng]
    public let bufferElevations: [Double]

    public init(points: [LatLng], elevations: [Double], elapsedSeconds: Int,
                paused: Bool = false, bufferPoints: [LatLng] = [], bufferElevations: [Double] = []) {
        self.points = points
        self.elevations = elevations
        self.elapsedSeconds = elapsedSeconds
        self.paused = paused
        self.bufferPoints = bufferPoints
        self.bufferElevations = bufferElevations
    }
}

/// Durable scratch store for the in-progress recording. The controller writes the accumulated
/// points (+ elevations + elapsed) as they grow and clears it on save/discard, so a relaunch
/// after a kill can resume the track. Mirrors Android's `RecordingDraftStore`.
public protocol RecordingDraftStore: Sendable {
    func load() async -> RecordingDraft?
    func save(_ draft: RecordingDraft) async
    func clear() async
}

/// No-op store (tests / previews) — nothing survives a kill.
public struct NoopRecordingDraftStore: RecordingDraftStore {
    public init() {}
    public func load() async -> RecordingDraft? { nil }
    public func save(_ draft: RecordingDraft) async {}
    public func clear() async {}
}

/// `UserDefaults`-backed store. Points/elevations are encoded as compact `;`-delimited strings
/// (same shape as Android's DataStore draft) so a recording survives process death.
public actor UserDefaultsRecordingDraftStore: RecordingDraftStore {
    private let defaults: UserDefaults
    private enum Key {
        static let points = "recording.draft.points"
        static let elevations = "recording.draft.elevations"
        static let elapsed = "recording.draft.elapsed"
        static let paused = "recording.draft.paused"
        static let bufferPoints = "recording.draft.bufferPoints"
        static let bufferElevations = "recording.draft.bufferElevations"
    }

    public init(defaults: UserDefaults = .standard) { self.defaults = defaults }

    private func encode(_ points: [LatLng]) -> String { points.map { "\($0.lat),\($0.lng)" }.joined(separator: ";") }
    private func decode(_ s: String?) -> [LatLng] {
        (s ?? "").split(separator: ";").compactMap { pair in
            let p = pair.split(separator: ",")
            guard p.count == 2, let lat = Double(p[0]), let lng = Double(p[1]) else { return nil }
            return LatLng(lat: lat, lng: lng)
        }
    }
    private func decodeDoubles(_ s: String?) -> [Double] {
        (s ?? "").split(separator: ";", omittingEmptySubsequences: false).compactMap { Double($0) }
    }

    public func load() async -> RecordingDraft? {
        let points = decode(defaults.string(forKey: Key.points))
        guard !points.isEmpty else { return nil }
        return RecordingDraft(
            points: points,
            elevations: decodeDoubles(defaults.string(forKey: Key.elevations)),
            elapsedSeconds: defaults.integer(forKey: Key.elapsed),
            paused: defaults.bool(forKey: Key.paused),
            bufferPoints: decode(defaults.string(forKey: Key.bufferPoints)),
            bufferElevations: decodeDoubles(defaults.string(forKey: Key.bufferElevations))
        )
    }

    public func save(_ draft: RecordingDraft) async {
        defaults.set(encode(draft.points), forKey: Key.points)
        defaults.set(draft.elevations.map { String($0) }.joined(separator: ";"), forKey: Key.elevations)
        defaults.set(draft.elapsedSeconds, forKey: Key.elapsed)
        defaults.set(draft.paused, forKey: Key.paused)
        defaults.set(encode(draft.bufferPoints), forKey: Key.bufferPoints)
        defaults.set(draft.bufferElevations.map { String($0) }.joined(separator: ";"), forKey: Key.bufferElevations)
    }

    public func clear() async {
        [Key.points, Key.elevations, Key.elapsed, Key.paused, Key.bufferPoints, Key.bufferElevations]
            .forEach { defaults.removeObject(forKey: $0) }
    }
}
