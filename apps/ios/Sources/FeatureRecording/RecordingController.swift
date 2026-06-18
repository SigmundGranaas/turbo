import Foundation
import Observation
import CoreModel
import CoreData

/// Owns the live track-recording **session** for the app's lifetime — accumulating
/// points, distance and elapsed time from ``LocationProvider`` fixes, driving the
/// background-location grant + Live Activity, and saving the result as a
/// ``SavedPath``. Mirrors Android's singleton `RecordingController` + foreground
/// service: the session must outlive any single screen, so the recording survives
/// the sheet being dismissed (the user can keep using the map while recording).
///
/// One instance lives in `AppContainer`; screens observe it, they don't own it.
@MainActor
@Observable
public final class RecordingController {
    public private(set) var isRecording = false
    public private(set) var pointCount = 0
    public private(set) var distanceMeters: Double = 0
    public private(set) var elapsedSeconds: Int = 0
    public private(set) var ascentMeters: Double = 0
    public private(set) var descentMeters: Double = 0
    public private(set) var currentAltitude: Double?
    public private(set) var currentSpeedMps: Double?
    public private(set) var maxSpeedMps: Double = 0
    /// Paused but still capturing into a side buffer (US-4) — distinct from a stopped session.
    public private(set) var isPausedBuffering = false
    /// Distance (m) walked while paused, pending Include/Discard on resume.
    public private(set) var bufferedDistanceM: Double = 0
    /// Whether enough was walked while paused to be worth asking about on resume.
    public var hasBufferedMovement: Bool { bufferedDistanceM >= CaptureSession.resumePromptM } // D4

    /// Average moving pace in seconds per kilometre, or nil before any distance.
    public var paceSecondsPerKm: Int? {
        guard distanceMeters > 50 else { return nil }
        return Int(Double(elapsedSeconds) / (distanceMeters / 1000))
    }

    /// The travelled track + pause-buffer, driven by the shared ``CaptureSession`` engine — the
    /// SAME one a *followed* route records with, so recording and following can't drift and pause
    /// behaves identically in both (Follow = Record, US-4).
    private var session = CaptureSession()
    private var startedAt: Date?
    // Paused time is excluded from the elapsed clock.
    private var pausedTotal: TimeInterval = 0
    private var pauseStartedAt: Date?

    private let location: LocationProvider
    private let pathRepository: PathRepository
    private let activity: RecordingActivityPresenter
    private let draftStore: RecordingDraftStore
    private let now: @Sendable () -> Date
    private var fixObservation: Task<Void, Never>?
    private var ticker: Task<Void, Never>?

    public init(
        location: LocationProvider,
        pathRepository: PathRepository,
        activity: RecordingActivityPresenter = NoRecordingActivityPresenter(),
        draftStore: RecordingDraftStore = NoopRecordingDraftStore(),
        now: @escaping @Sendable () -> Date = { Date() }
    ) {
        self.location = location
        self.pathRepository = pathRepository
        self.activity = activity
        self.draftStore = draftStore
        self.now = now
    }

    /// True while a track exists — recording OR stopped-but-unsaved. Drives the
    /// map's "recording" pill so the session is visible while the sheet is closed.
    public var isSessionActive: Bool { startedAt != nil }
    /// A session exists but isn't currently capturing (the "paused" state).
    public var isPaused: Bool { isSessionActive && !isRecording }

    /// Begin a new recording.
    public func start() {
        guard !isRecording else { return }
        session = CaptureSession()
        isPausedBuffering = false; bufferedDistanceM = 0; pausedTotal = 0; pauseStartedAt = nil
        pointCount = 0; distanceMeters = 0; elapsedSeconds = 0
        ascentMeters = 0; descentMeters = 0; currentAltitude = nil
        currentSpeedMps = nil; maxSpeedMps = 0
        startedAt = now()
        // Always-auth keeps the track alive when the phone is pocketed or locked —
        // the iOS analogue of Android's foreground service. Only needed on a fresh
        // start; a resume reuses the existing grant.
        location.requestAlwaysAuthorization()
        beginObserving()
    }

    /// Resume a stopped-but-unsaved recording (the "Keep Recording" path) WITHOUT
    /// discarding the captured track — unlike ``start()``, which begins anew.
    public func resume() {
        guard !isRecording, startedAt != nil else { return }
        beginObserving()
    }

    private func beginObserving() {
        isRecording = true
        location.setBackgroundUpdates(true)
        activity.begin(title: "Recording")

        fixObservation = Task { [weak self, location] in
            // Resume any persisted draft (after a process kill) before collecting new fixes.
            if let draft = await self?.draftStore.load() {
                self?.restore(from: draft)
            }
            for await fix in location.fixes() {
                self?.append(fix)
            }
        }
        ticker = Task { [weak self] in
            while !Task.isCancelled {
                try? await Task.sleep(for: .seconds(1))
                guard let self, let started = self.startedAt, self.isRecording else { continue }
                // Paused time is excluded — the clock freezes while buffering and resumes after.
                let pausing = self.pauseStartedAt.map { self.now().timeIntervalSince($0) } ?? 0
                self.elapsedSeconds = Int(self.now().timeIntervalSince(started) - self.pausedTotal - pausing)
                self.activity.update(distanceMeters: self.distanceMeters, elapsedSeconds: self.elapsedSeconds)
            }
        }
    }

    /// Stop accumulating but keep the captured track (pending save / discard).
    public func stop() {
        isRecording = false
        location.setBackgroundUpdates(false)
        activity.end()
        fixObservation?.cancel(); fixObservation = nil
        ticker?.cancel(); ticker = nil
    }

    /// Persist the captured track. Blank names fall back to a dated default.
    public func save(name: String) {
        let trimmed = name.trimmingCharacters(in: .whitespacesAndNewlines)
        let track = session.track
        guard !track.points.isEmpty else { reset(); return }
        let path = SavedPath(
            id: "rec-\(UUID().uuidString)",
            name: trimmed.isEmpty ? "Recorded track" : trimmed,
            path: GeoPath(
                points: track.points,
                source: .recording,
                elevations: track.elevations.isEmpty ? nil : track.elevations,
                distanceM: track.distanceM, // explicit so a Discard pen-up gap isn't recounted
                recordedAtEpochMs: startedAt.map { Int64($0.timeIntervalSince1970 * 1000) },
                movingTimeSeconds: elapsedSeconds
            ),
            activityKind: .hiking
        )
        let repository = pathRepository
        Task { await repository.upsert(path) }
        reset()
    }

    /// Throw the captured track away.
    public func discard() { stop(); reset() }

    private func append(_ fix: LocationFix) {
        guard isRecording else { return }
        // The shared engine folds the fix into the track, or — while paused — into the side
        // buffer (so a forgotten unpause never silently loses the walk, US-4).
        session = session.appending(fix)
        if session.paused {
            bufferedDistanceM = session.bufferedDistanceM
            // Persist the track AND the buffer so a kill-while-paused doesn't lose the walk.
            let main = session.track, buf = session.buffer, elapsed = elapsedSeconds
            Task {
                await draftStore.save(RecordingDraft(
                    points: main.points, elevations: main.elevations, elapsedSeconds: elapsed,
                    paused: true, bufferPoints: buf.points, bufferElevations: buf.elevations
                ))
            }
            return
        }
        publish(from: session.track)
        activity.update(distanceMeters: distanceMeters, elapsedSeconds: elapsedSeconds)
        // Persist the growing track so it survives process death.
        let snapshot = session.track
        let elapsed = elapsedSeconds
        Task { await draftStore.save(RecordingDraft(points: snapshot.points, elevations: snapshot.elevations, elapsedSeconds: elapsed)) }
    }

    /// Restore a recording from a persisted draft after a relaunch — unless live fixes already
    /// landed (don't clobber them). The clock continues from the draft's elapsed time.
    private func restore(from draft: RecordingDraft) {
        guard session.track.points.isEmpty, !draft.points.isEmpty else { return }
        let track = CapturedTrack(
            points: draft.points,
            elevations: draft.elevations,
            distanceM: GeoMetrics.pathLengthMeters(draft.points),
            ascentM: GeoMetrics.ascentMeters(draft.elevations) ?? 0,
            descentM: GeoMetrics.descentMeters(draft.elevations) ?? 0,
            currentAltitude: draft.elevations.last
        )
        session = CaptureSession(track: track)
        startedAt = now().addingTimeInterval(-Double(draft.elapsedSeconds))
        elapsedSeconds = draft.elapsedSeconds
        publish(from: session.track)
        // Killed while PAUSED: restore the held buffer and stay paused (the clock stays frozen).
        if draft.paused {
            let buffer = CapturedTrack(
                points: draft.bufferPoints,
                elevations: draft.bufferElevations,
                distanceM: GeoMetrics.pathLengthMeters(draft.bufferPoints),
                currentAltitude: draft.bufferElevations.last
            )
            session = CaptureSession(track: track, paused: true, buffer: buffer, pauseAnchor: track.points.last)
            isPausedBuffering = true
            pauseStartedAt = now()
            bufferedDistanceM = session.bufferedDistanceM
        }
    }

    /// Pause the recording; capture continues into a side buffer (US-4). The clock freezes.
    public func pause() {
        guard isRecording, !isPausedBuffering else { return }
        session = session.paused()
        isPausedBuffering = true
        bufferedDistanceM = 0
        pauseStartedAt = now()
    }

    /// Resume from a pause. `includeBuffered` stitches the walk captured while paused onto the
    /// track (it IS the path you walked); otherwise it's discarded and the pen lifted so the gap
    /// isn't counted. Either way the buffer clears and the clock resumes.
    public func resume(includeBuffered: Bool) {
        guard isPausedBuffering else { return }
        session = session.resuming(include: includeBuffered)
        publish(from: session.track)
        if let ps = pauseStartedAt { pausedTotal += now().timeIntervalSince(ps); pauseStartedAt = nil }
        isPausedBuffering = false
        bufferedDistanceM = 0
    }

    private func publish(from c: CapturedTrack) {
        pointCount = c.pointCount
        distanceMeters = c.distanceM
        ascentMeters = c.ascentM
        descentMeters = c.descentM
        currentAltitude = c.currentAltitude
        currentSpeedMps = c.currentSpeedMps
        maxSpeedMps = c.maxSpeedMps
    }

    private func reset() {
        stop()
        Task { await draftStore.clear() }
        session = CaptureSession(); startedAt = nil
        isPausedBuffering = false; bufferedDistanceM = 0; pausedTotal = 0; pauseStartedAt = nil
        pointCount = 0; distanceMeters = 0; elapsedSeconds = 0
        ascentMeters = 0; descentMeters = 0; currentAltitude = nil
        currentSpeedMps = nil; maxSpeedMps = 0
    }
}
