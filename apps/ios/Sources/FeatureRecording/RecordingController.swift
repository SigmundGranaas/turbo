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

    /// Average moving pace in seconds per kilometre, or nil before any distance.
    public var paceSecondsPerKm: Int? {
        guard distanceMeters > 50 else { return nil }
        return Int(Double(elapsedSeconds) / (distanceMeters / 1000))
    }

    /// The travelled track, accumulated by the shared ``TrackCapture`` engine — the
    /// same one a *followed* route records with, so the two can't drift (Follow = Record).
    private var capture = CapturedTrack()
    private var startedAt: Date?

    private let location: LocationProvider
    private let pathRepository: PathRepository
    private let activity: RecordingActivityPresenter
    private let now: @Sendable () -> Date
    private var fixObservation: Task<Void, Never>?
    private var ticker: Task<Void, Never>?

    public init(
        location: LocationProvider,
        pathRepository: PathRepository,
        activity: RecordingActivityPresenter = NoRecordingActivityPresenter(),
        now: @escaping @Sendable () -> Date = { Date() }
    ) {
        self.location = location
        self.pathRepository = pathRepository
        self.activity = activity
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
        capture = CapturedTrack()
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
            for await fix in location.fixes() {
                self?.append(fix)
            }
        }
        ticker = Task { [weak self] in
            while !Task.isCancelled {
                try? await Task.sleep(for: .seconds(1))
                guard let self, let started = self.startedAt, self.isRecording else { continue }
                self.elapsedSeconds = Int(self.now().timeIntervalSince(started))
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
        guard !capture.points.isEmpty else { reset(); return }
        let path = SavedPath(
            id: "rec-\(UUID().uuidString)",
            name: trimmed.isEmpty ? "Recorded track" : trimmed,
            path: GeoPath(
                points: capture.points,
                source: .recording,
                elevations: capture.elevations.isEmpty ? nil : capture.elevations,
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
        capture = TrackCapture.append(capture, fix)
        pointCount = capture.pointCount
        distanceMeters = capture.distanceM
        ascentMeters = capture.ascentM
        descentMeters = capture.descentM
        currentAltitude = capture.currentAltitude
        currentSpeedMps = capture.currentSpeedMps
        maxSpeedMps = capture.maxSpeedMps
        activity.update(distanceMeters: distanceMeters, elapsedSeconds: elapsedSeconds)
    }

    private func reset() {
        stop()
        capture = CapturedTrack(); startedAt = nil
        pointCount = 0; distanceMeters = 0; elapsedSeconds = 0
        ascentMeters = 0; descentMeters = 0; currentAltitude = nil
        currentSpeedMps = nil; maxSpeedMps = 0
    }
}
