import Foundation
import CoreModel

/// A recording snapshot persisted across process death so a track survives an app kill.
/// Mirrors Android's `RecordingDraft`.
public struct RecordingDraft: Sendable, Equatable {
    public let points: [LatLng]
    public let elevations: [Double]
    public let elapsedSeconds: Int

    public init(points: [LatLng], elevations: [Double], elapsedSeconds: Int) {
        self.points = points
        self.elevations = elevations
        self.elapsedSeconds = elapsedSeconds
    }
}

/// Durable scratch store for the in-progress recording. The controller writes the accumulated
/// points (+ elevations + elapsed) as they grow and clears it on save/discard, so a relaunch
/// after a kill can resume the track. Mirrors Android's `RecordingDraftStore`.
public protocol RecordingDraftStore: Sendable {
    func load() async -> RecordingDraft?
    func save(points: [LatLng], elevations: [Double], elapsedSeconds: Int) async
    func clear() async
}

/// No-op store (tests / previews) — nothing survives a kill.
public struct NoopRecordingDraftStore: RecordingDraftStore {
    public init() {}
    public func load() async -> RecordingDraft? { nil }
    public func save(points: [LatLng], elevations: [Double], elapsedSeconds: Int) async {}
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
    }

    public init(defaults: UserDefaults = .standard) { self.defaults = defaults }

    public func load() async -> RecordingDraft? {
        guard let encoded = defaults.string(forKey: Key.points), !encoded.isEmpty else { return nil }
        let points = encoded.split(separator: ";").compactMap { pair -> LatLng? in
            let parts = pair.split(separator: ",")
            guard parts.count == 2, let lat = Double(parts[0]), let lng = Double(parts[1]) else { return nil }
            return LatLng(lat: lat, lng: lng)
        }
        guard !points.isEmpty else { return nil }
        let elevations = (defaults.string(forKey: Key.elevations) ?? "")
            .split(separator: ";", omittingEmptySubsequences: false)
            .compactMap { Double($0) }
        return RecordingDraft(points: points, elevations: elevations, elapsedSeconds: defaults.integer(forKey: Key.elapsed))
    }

    public func save(points: [LatLng], elevations: [Double], elapsedSeconds: Int) async {
        defaults.set(points.map { "\($0.lat),\($0.lng)" }.joined(separator: ";"), forKey: Key.points)
        defaults.set(elevations.map { String($0) }.joined(separator: ";"), forKey: Key.elevations)
        defaults.set(elapsedSeconds, forKey: Key.elapsed)
    }

    public func clear() async {
        defaults.removeObject(forKey: Key.points)
        defaults.removeObject(forKey: Key.elevations)
        defaults.removeObject(forKey: Key.elapsed)
    }
}
