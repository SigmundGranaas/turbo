import Foundation

/// A snapshot of the active recording session, passed to ``MapScreen`` for its
/// ambient "recording" pill. A plain value so `FeatureMap` needn't depend on
/// `FeatureRecording` — the root maps the controller's state into this.
public struct RecordingStatus: Equatable, Sendable {
    public let isRecording: Bool
    public let distanceMeters: Double
    public let elapsedSeconds: Int

    public init(isRecording: Bool, distanceMeters: Double, elapsedSeconds: Int) {
        self.isRecording = isRecording
        self.distanceMeters = distanceMeters
        self.elapsedSeconds = elapsedSeconds
    }

    /// `"12:34 · 1.2 km"` — elapsed then distance.
    public var label: String {
        let s = elapsedSeconds
        let time = s >= 3600
            ? String(format: "%d:%02d:%02d", s / 3600, (s % 3600) / 60, s % 60)
            : String(format: "%02d:%02d", s / 60, s % 60)
        return "\(time) · \(String(format: "%.1f km", distanceMeters / 1000))"
    }
}
