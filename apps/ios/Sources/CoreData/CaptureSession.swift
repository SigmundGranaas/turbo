import Foundation
import CoreModel

/// The travelled-track capture state machine shared by recording AND following
/// (Follow = Record). It wraps the running ``CapturedTrack`` with the pause-buffer
/// behaviour (US-4): while paused, fixes accumulate into a side ``buffer`` anchored to
/// the last track point instead of the track itself, so forgetting to unpause never
/// silently loses the walk; on resume the caller chooses Include (stitch the buffer in,
/// back-dated) or Discard (drop it and lift the pen so the gap isn't counted).
///
/// Pure value type: both `RecordingController` and `FollowController` hold one and apply
/// transitions, which is what makes pause behave identically in either mode. Mirrors
/// Android `CaptureSession`. Pinned by `CaptureSessionTests`.
public struct CaptureSession: Equatable, Sendable {
    /// The committed track — the line that has actually been walked and counted.
    public var track: CapturedTrack
    public var paused: Bool
    /// Movement captured while paused, not yet folded into ``track``.
    public var buffer: CapturedTrack
    /// The track point at the moment of pausing — buffered distance is measured from here.
    public var pauseAnchor: LatLng?
    /// After a Discard resume the next live fix starts a fresh segment (no gap distance).
    public var penUpNext: Bool

    public init(
        track: CapturedTrack = CapturedTrack(),
        paused: Bool = false,
        buffer: CapturedTrack = CapturedTrack(),
        pauseAnchor: LatLng? = nil,
        penUpNext: Bool = false
    ) {
        self.track = track
        self.paused = paused
        self.buffer = buffer
        self.pauseAnchor = pauseAnchor
        self.penUpNext = penUpNext
    }

    /// Buffered walking past this (m) prompts Include/Discard on resume; below it, just resume (D4).
    public static let resumePromptM = 80.0

    /// Distance (m) walked while paused, measured from ``pauseAnchor`` so the very first paused fix
    /// already counts; 0 when idle.
    public var bufferedDistanceM: Double {
        guard let first = buffer.points.first else { return 0 }
        let join = pauseAnchor.map { GeoMetrics.haversineMeters($0, first) } ?? 0
        return join + buffer.distanceM
    }

    /// Whether enough was walked while paused to be worth asking about on resume.
    public var hasBufferedMovement: Bool { bufferedDistanceM >= Self.resumePromptM }

    /// Fold one (already accuracy/jump-filtered) fix in: into the side ``buffer`` while paused,
    /// else into the ``track`` (detaching the first post-Discard fix so the gap isn't counted).
    public func appending(_ fix: LocationFix) -> CaptureSession {
        var s = self
        if paused {
            s.buffer = TrackCapture.append(s.buffer, fix)
            return s
        }
        s.track = penUpNext ? TrackCapture.appendDetached(s.track, fix) : TrackCapture.append(s.track, fix)
        s.penUpNext = false
        return s
    }

    /// Begin a pause: capture continues into a fresh buffer anchored at the last track point.
    public func paused(_ on: Bool = true) -> CaptureSession {
        guard on, !paused else { return self }
        var s = self
        s.paused = true
        s.buffer = CapturedTrack()
        s.pauseAnchor = track.points.last
        return s
    }

    /// Resume from a pause. `include` = true stitches the buffered walk onto the track (back-dated);
    /// false drops it and lifts the pen so the gap back to the pre-pause point isn't counted. Either
    /// way the buffer is cleared.
    public func resuming(include: Bool) -> CaptureSession {
        guard paused else { return self }
        var s = self
        if include, let first = buffer.points.first {
            let join = track.points.last.map { GeoMetrics.haversineMeters($0, first) } ?? 0
            s.track.points += buffer.points
            s.track.elevations += buffer.elevations
            s.track.distanceM += join + buffer.distanceM
            s.track.maxSpeedMps = max(track.maxSpeedMps, buffer.maxSpeedMps)
            s.track.ascentM = GeoMetrics.ascentMeters(s.track.elevations) ?? s.track.ascentM
            s.track.descentM = GeoMetrics.descentMeters(s.track.elevations) ?? s.track.descentM
            s.track.currentAltitude = buffer.currentAltitude ?? track.currentAltitude
            s.penUpNext = false
        } else {
            s.penUpNext = true
        }
        s.paused = false
        s.buffer = CapturedTrack()
        s.pauseAnchor = nil
        return s
    }
}
