import Foundation
import CoreModel

/// The travelled track accumulated from GPS fixes — the shared capture core of BOTH
/// recording and following (Follow = Record). Mirrors exactly what `RecordingController`
/// accumulated inline, so a *followed* route saves a track identical to a *recording* of
/// the same fixes. Pure value type; the owner holds the running track and folds each fix
/// in with `TrackCapture.append`. Pinned by `TrackCaptureTests`.
public struct CapturedTrack: Equatable, Sendable {
    public var points: [LatLng]
    /// Per-fix altitudes — appended only when a fix carried altitude (so it parallels the
    /// fixes that had elevation, matching the recording engine's long-standing behaviour).
    public var elevations: [Double]
    public var distanceM: Double
    public var ascentM: Double
    public var descentM: Double
    public var currentAltitude: Double?
    public var currentSpeedMps: Double?
    public var maxSpeedMps: Double

    public init(
        points: [LatLng] = [],
        elevations: [Double] = [],
        distanceM: Double = 0,
        ascentM: Double = 0,
        descentM: Double = 0,
        currentAltitude: Double? = nil,
        currentSpeedMps: Double? = nil,
        maxSpeedMps: Double = 0
    ) {
        self.points = points
        self.elevations = elevations
        self.distanceM = distanceM
        self.ascentM = ascentM
        self.descentM = descentM
        self.currentAltitude = currentAltitude
        self.currentSpeedMps = currentSpeedMps
        self.maxSpeedMps = maxSpeedMps
    }

    public var pointCount: Int { points.count }
    public var userLocation: LatLng? { points.last }
}

/// Folds GPS fixes into a ``CapturedTrack``. Stateless — the caller holds the track.
public enum TrackCapture {
    /// Fold one (already accuracy/jump-filtered) fix into `track`. Every fix becomes a
    /// point; distance/ascent/descent recompute over the running series; elevation is
    /// recorded only when the fix carries altitude; speed refreshes (and peaks) whenever
    /// the fix carries it. This is byte-for-byte what the record screen does, which is
    /// exactly the Follow = Record parity US-1 promises.
    public static func append(_ track: CapturedTrack, _ fix: LocationFix) -> CapturedTrack {
        let prev = track.points.last
        var t = folded(track, fix)
        // Incremental distance (== pathLengthMeters for a continuous walk) so a detached
        // segment after a paused-Discard can skip its connecting gap.
        if let prev { t.distanceM += GeoMetrics.haversineMeters(prev, fix.position) }
        return t
    }

    /// Like ``append`` but starts a fresh segment — the new point adds NO connecting distance
    /// from the previous one. Used after a paused-buffer **Discard** (US-4) so the gap you
    /// walked while paused isn't counted, even though the polyline still joins the two ends.
    public static func appendDetached(_ track: CapturedTrack, _ fix: LocationFix) -> CapturedTrack {
        folded(track, fix)
    }

    /// Shared fold: append the point/altitude/speed and recompute ascent/descent. Distance is
    /// applied by the caller (``append`` adds the step; ``appendDetached`` leaves it).
    private static func folded(_ track: CapturedTrack, _ fix: LocationFix) -> CapturedTrack {
        var t = track
        t.points.append(fix.position)
        if let altitude = fix.altitude {
            t.elevations.append(altitude)
            t.currentAltitude = altitude
        }
        t.ascentM = GeoMetrics.ascentMeters(t.elevations) ?? t.ascentM
        t.descentM = GeoMetrics.descentMeters(t.elevations) ?? t.descentM
        if let speed = fix.speedMps {
            t.currentSpeedMps = speed
            t.maxSpeedMps = max(t.maxSpeedMps, speed)
        }
        return t
    }
}
