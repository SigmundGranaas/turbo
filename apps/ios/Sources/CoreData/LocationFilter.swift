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
    private var lastAccepted: LatLng?
    private var pendingJump: LatLng?

    public init(accuracyMaxM: Double = 50, stalenessMaxMs: Double = 5000, jumpMaxM: Double = 200) {
        self.accuracyMaxM = accuracyMaxM
        self.stalenessMaxMs = stalenessMaxMs
        self.jumpMaxM = jumpMaxM
    }

    /// Whether to accept this fix (and advance the filter state).
    /// - `accuracyM`: horizontal accuracy in metres (CoreLocation `horizontalAccuracy`).
    /// - `ageMs`: how old the fix is (now − fix timestamp), in milliseconds.
    public func accept(position: LatLng, accuracyM: Double, ageMs: Double) -> Bool {
        if accuracyM > accuracyMaxM { return false }
        if ageMs > stalenessMaxMs { return false }

        guard let last = lastAccepted else {
            lastAccepted = position; pendingJump = nil
            return true
        }
        if GeoMetrics.haversineMeters(last, position) <= jumpMaxM {
            lastAccepted = position; pendingJump = nil
            return true
        }
        // A big jump — accept only if a second consistent fix confirms it (a real
        // fast move), otherwise it's a one-off glitch.
        if let pending = pendingJump, GeoMetrics.haversineMeters(pending, position) <= jumpMaxM {
            lastAccepted = position; pendingJump = nil
            return true
        }
        pendingJump = position
        return false
    }
}
