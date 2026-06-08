import Foundation
import Observation
import CoreModel
import CoreData

/// Records a live track from ``LocationProvider`` fixes — accumulating points,
/// distance and elapsed time — then saves it as a ``SavedPath``. Mirrors
/// `feature.recording.RecordingViewModel` (Android).
@MainActor
@Observable
public final class RecordingViewModel {
    public private(set) var isRecording = false
    public private(set) var pointCount = 0
    public private(set) var distanceMeters: Double = 0
    public private(set) var elapsedSeconds: Int = 0

    private var points: [LatLng] = []
    private var elevations: [Double] = []
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

    /// Begin a new recording.
    public func start() {
        guard !isRecording else { return }
        points = []; elevations = []
        pointCount = 0; distanceMeters = 0; elapsedSeconds = 0
        startedAt = now()
        isRecording = true
        // Always-auth + background updates keep the track alive when the phone is
        // pocketed or locked — the iOS analogue of Android's foreground service.
        location.requestAlwaysAuthorization()
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
        guard !points.isEmpty else { reset(); return }
        let path = SavedPath(
            id: "rec-\(UUID().uuidString)",
            name: trimmed.isEmpty ? "Recorded track" : trimmed,
            path: GeoPath(
                points: points,
                source: .recording,
                elevations: elevations.isEmpty ? nil : elevations,
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
        points.append(fix.position)
        if let altitude = fix.altitude { elevations.append(altitude) }
        pointCount = points.count
        distanceMeters = GeoMetrics.pathLengthMeters(points)
        activity.update(distanceMeters: distanceMeters, elapsedSeconds: elapsedSeconds)
    }

    private func reset() {
        stop()
        points = []; elevations = []; startedAt = nil
        pointCount = 0; distanceMeters = 0; elapsedSeconds = 0
    }
}
