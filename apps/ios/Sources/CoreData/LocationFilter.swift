import Foundation
import CoreModel

/// Gates raw location fixes before any consumer (map dot, recording, following)
/// sees them, so a stale or wildly-inaccurate reading can't teleport the user.
/// Stateful (holds the last accepted fix). Mirrors the Android `LocationFilter`;
/// both are pinned by `fixtures/tracking/filter/*`.
public final class LocationFilter {
    private let accuracyMaxM: Double
    private let stalenessMaxMs: Double
    private let jumpMaxM: Double
    private let maxSpeedMps: Double
    private var lastAccepted: LatLng?
    private var pendingJump: LatLng?

    public init(accuracyMaxM: Double = 50, stalenessMaxMs: Double = 5000, jumpMaxM: Double = 200, maxSpeedMps: Double = 30) {
        self.accuracyMaxM = accuracyMaxM
        self.stalenessMaxMs = stalenessMaxMs
        self.jumpMaxM = jumpMaxM
        self.maxSpeedMps = maxSpeedMps
    }

    /// Whether to accept this fix (and advance the filter state).
    /// - `accuracyM`: horizontal accuracy in metres (CoreLocation `horizontalAccuracy`).
    /// - `ageMs`: how old the fix is (now − fix timestamp), in milliseconds.
    /// - `intervalMs`: time since the previous fix (drives the speed gate); the 1 s default
    ///   keeps the gate sane when the caller can't supply it (e.g. shared fixtures).
    public func accept(position: LatLng, accuracyM: Double, ageMs: Double, intervalMs: Double = 1000) -> Bool {
        if accuracyM > accuracyMaxM { return false }
        if ageMs > stalenessMaxMs { return false }

        guard let last = lastAccepted else {
            lastAccepted = position; pendingJump = nil
            return true
        }
        if !isJump(last, position, intervalMs) {
            lastAccepted = position; pendingJump = nil
            return true
        }
        // A jump — accept only if a second consistent fix confirms it (a real fast move),
        // otherwise it's a one-off glitch.
        if let pending = pendingJump, !isJump(pending, position, intervalMs) {
            lastAccepted = position; pendingJump = nil
            return true
        }
        pendingJump = position
        return false
    }

    /// A fix is a jump if it's too far (absolute) OR implies an implausible speed (rate, D5).
    private func isJump(_ from: LatLng, _ to: LatLng, _ intervalMs: Double) -> Bool {
        let d = GeoMetrics.haversineMeters(from, to)
        if d > jumpMaxM { return true }
        let seconds = max(intervalMs / 1000, 0.001)
        return d / seconds > maxSpeedMps
    }
}
