#if canImport(ActivityKit) && os(iOS)
import ActivityKit
import Foundation

/// The ActivityKit contract for the "recording a track" Live Activity — shared
/// between the app (which starts/updates/ends the activity) and the widget
/// extension (which renders it on the Lock Screen + Dynamic Island). The iOS
/// analogue of the Android recording foreground-service notification.
///
/// Entirely `#if canImport(ActivityKit)`-guarded so the macOS host build (used by
/// `swift test`) sees an empty module.
public struct RecordingActivityAttributes: ActivityAttributes {
    /// The live, changing values pushed on every update.
    public struct ContentState: Codable, Hashable, Sendable {
        public var distanceMeters: Double
        public var elapsedSeconds: Int
        public init(distanceMeters: Double, elapsedSeconds: Int) {
            self.distanceMeters = distanceMeters
            self.elapsedSeconds = elapsedSeconds
        }

        /// `"3.2 km"` — kilometres with one decimal.
        public var formattedDistance: String { String(format: "%.1f km", distanceMeters / 1000) }

        /// `"1:04:09"` / `"04:09"` — hours only when non-zero.
        public var formattedElapsed: String {
            let h = elapsedSeconds / 3600, m = (elapsedSeconds % 3600) / 60, s = elapsedSeconds % 60
            return h > 0 ? String(format: "%d:%02d:%02d", h, m, s) : String(format: "%02d:%02d", m, s)
        }
    }

    /// Static for the lifetime of the activity — the track's name.
    public var title: String
    public init(title: String) { self.title = title }
}
#endif
