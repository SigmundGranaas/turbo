import Foundation
import Testing
import CoreModel
import CoreData
@testable import FeatureRecording

@Suite("RecordingController")
@MainActor
struct RecordingControllerTests {

    private func provider() -> SimulatedLocationProvider {
        SimulatedLocationProvider(fixes: [
            LocationFix(position: LatLng(lat: 69.60, lng: 19.90), altitude: 10),
            LocationFix(position: LatLng(lat: 69.61, lng: 19.92), altitude: 24),
            LocationFix(position: LatLng(lat: 69.62, lng: 19.95), altitude: 18),
        ])
    }

    @Test("recording accumulates points + distance from location fixes")
    func accumulates() async {
        let vm = RecordingController(location: provider(), pathRepository: InMemoryPathRepository(seed: []))
        vm.start()
        #expect(vm.isRecording)
        try? await Task.sleep(for: .milliseconds(250))
        #expect(vm.pointCount == 3)
        #expect(vm.distanceMeters > 0)
    }

    /// A longer, slower walk so a pause can land between fixes.
    private func walk() -> SimulatedLocationProvider {
        let fixes = (0...7).map { LocationFix(position: LatLng(lat: 69.60 + Double($0) * 0.001, lng: 19.90), altitude: 10) }
        return SimulatedLocationProvider(fixes: fixes, interval: .milliseconds(40))
    }

    @Test("pausing buffers the walk instead of dropping it; Include merges it (US-4)")
    func pauseInclude() async {
        let vm = RecordingController(location: walk(), pathRepository: InMemoryPathRepository(seed: []))
        vm.start()
        try? await Task.sleep(for: .milliseconds(110))   // a couple of fixes land on the track
        let trackDist = vm.distanceMeters
        let trackPts = vm.pointCount
        vm.pause()
        #expect(vm.isPausedBuffering)
        try? await Task.sleep(for: .milliseconds(240))   // the rest arrive while paused → buffer
        #expect(vm.distanceMeters == trackDist)          // track untouched while paused
        #expect(vm.pointCount == trackPts)
        #expect(vm.bufferedDistanceM > 90)               // the paused walk was captured
        #expect(vm.hasBufferedMovement)
        vm.resume(includeBuffered: true)
        #expect(vm.isPausedBuffering == false)
        #expect(vm.bufferedDistanceM == 0)
        #expect(vm.distanceMeters > trackDist)           // buffer stitched onto the track
        #expect(vm.pointCount > trackPts)
    }

    @Test("pausing then Discard keeps the track as it was")
    func pauseDiscard() async {
        let vm = RecordingController(location: walk(), pathRepository: InMemoryPathRepository(seed: []))
        vm.start()
        try? await Task.sleep(for: .milliseconds(110))
        let trackDist = vm.distanceMeters
        let trackPts = vm.pointCount
        vm.pause()
        try? await Task.sleep(for: .milliseconds(240))
        vm.resume(includeBuffered: false)
        #expect(vm.isPausedBuffering == false)
        #expect(vm.distanceMeters == trackDist)          // the paused walk was discarded
        #expect(vm.pointCount == trackPts)
    }

    @Test("a persisted draft is resumed on start (process-death recovery)")
    func draftResume() async {
        let defaults = UserDefaults(suiteName: "draft.test.\(UUID().uuidString)")!
        let store = UserDefaultsRecordingDraftStore(defaults: defaults)
        await store.save(
            points: [LatLng(lat: 69.0, lng: 18.0), LatLng(lat: 69.001, lng: 18.0)],
            elevations: [10, 25], elapsedSeconds: 42
        )
        // Round-trips through UserDefaults…
        let loaded = await store.load()
        #expect(loaded?.points.count == 2)
        #expect(loaded?.elevations == [10, 25])
        #expect(loaded?.elapsedSeconds == 42)

        // …and a controller started with it resumes the track + clock.
        let vm = RecordingController(location: SimulatedLocationProvider(fixes: []),
                                     pathRepository: InMemoryPathRepository(seed: []), draftStore: store)
        vm.start()
        try? await Task.sleep(for: .milliseconds(150))
        #expect(vm.pointCount == 2)
        #expect(vm.distanceMeters > 90)
        #expect(vm.elapsedSeconds >= 42)
    }

    @Test("stop keeps the captured track; save persists it as a SavedPath")
    func saveTrack() async {
        let repo = InMemoryPathRepository(seed: [])
        let vm = RecordingController(location: provider(), pathRepository: repo)
        vm.start()
        try? await Task.sleep(for: .milliseconds(250))
        vm.stop()
        #expect(vm.isRecording == false)
        #expect(vm.pointCount == 3)

        vm.save(name: "Morning hike")
        try? await Task.sleep(for: .milliseconds(150))
        let saved = await repo.current()
        #expect(saved.count == 1)
        #expect(saved[0].name == "Morning hike")
        #expect(saved[0].path.points.count == 3)
        #expect(saved[0].path.source == .recording)
        #expect(vm.pointCount == 0)   // reset after save
    }

    @Test("discard throws the track away without saving")
    func discard() async {
        let repo = InMemoryPathRepository(seed: [])
        let vm = RecordingController(location: provider(), pathRepository: repo)
        vm.start()
        try? await Task.sleep(for: .milliseconds(250))
        vm.discard()
        try? await Task.sleep(for: .milliseconds(100))
        #expect(await repo.current().isEmpty)
        #expect(vm.isRecording == false)
        #expect(vm.pointCount == 0)
    }

    @Test("a blank name falls back to a default")
    func blankName() async {
        let repo = InMemoryPathRepository(seed: [])
        let vm = RecordingController(location: provider(), pathRepository: repo)
        vm.start()
        try? await Task.sleep(for: .milliseconds(250))
        vm.stop()
        vm.save(name: "   ")
        try? await Task.sleep(for: .milliseconds(150))
        #expect(await repo.current().first?.name.isEmpty == false)
    }

    @Test("recording asks for background updates; stopping turns them off")
    func backgroundLifecycle() async {
        let spy = BackgroundSpyProvider()
        let vm = RecordingController(location: spy, pathRepository: InMemoryPathRepository(seed: []))
        vm.start()
        #expect(spy.alwaysAuthorizationRequested)
        #expect(spy.backgroundUpdates == true)
        vm.stop()
        #expect(spy.backgroundUpdates == false)
    }

    @Test("keep recording resumes without discarding the captured track")
    func resumeKeepsTrack() async {
        let vm = RecordingController(location: provider(), pathRepository: InMemoryPathRepository(seed: []))
        vm.start()
        try? await Task.sleep(for: .milliseconds(250))
        let captured = vm.pointCount
        #expect(captured == 3)
        vm.stop()
        #expect(vm.isRecording == false)
        vm.resume()
        #expect(vm.isRecording)
        #expect(vm.pointCount >= captured)   // track preserved, NOT reset to 0
    }

    @Test("live stats: ascent, descent, speed accumulate from fixes")
    func liveStats() async {
        let fixes = [
            LocationFix(position: LatLng(lat: 69.60, lng: 19.90), altitude: 100, speedMps: 1.0),
            LocationFix(position: LatLng(lat: 69.61, lng: 19.92), altitude: 130, speedMps: 2.5),  // +30 climb
            LocationFix(position: LatLng(lat: 69.62, lng: 19.95), altitude: 110, speedMps: 1.5),  // -20 descend
        ]
        let vm = RecordingController(location: SimulatedLocationProvider(fixes: fixes),
                                     pathRepository: InMemoryPathRepository(seed: []))
        vm.start()
        try? await Task.sleep(for: .milliseconds(250))
        #expect(abs(vm.ascentMeters - 30) < 0.001)
        #expect(abs(vm.descentMeters - 20) < 0.001)
        #expect(vm.currentAltitude == 110)
        #expect(vm.maxSpeedMps == 2.5)
        #expect(vm.currentSpeedMps == 1.5)
    }

    @Test("session stays active across stop, clears on save/discard")
    func sessionLifetime() async {
        let vm = RecordingController(location: provider(), pathRepository: InMemoryPathRepository(seed: []))
        #expect(vm.isSessionActive == false)
        vm.start()
        #expect(vm.isSessionActive)              // recording
        try? await Task.sleep(for: .milliseconds(250))
        vm.stop()
        #expect(vm.isSessionActive)              // paused-but-unsaved → still active (drives the map pill)
        #expect(vm.isPaused)
        vm.save(name: "Hike")
        #expect(vm.isSessionActive == false)     // cleared once saved
    }

    @Test("recording begins a Live Activity and ends it on stop")
    func liveActivityLifecycle() async {
        let presenter = ActivitySpy()
        let vm = RecordingController(
            location: provider(),
            pathRepository: InMemoryPathRepository(seed: []),
            activity: presenter
        )
        vm.start()
        #expect(presenter.began)
        try? await Task.sleep(for: .milliseconds(250))
        #expect(presenter.updateCount > 0)   // pushed live stats as fixes arrived
        vm.stop()
        #expect(presenter.ended)
    }
}

@MainActor
private final class ActivitySpy: RecordingActivityPresenter {
    private(set) var began = false
    private(set) var ended = false
    private(set) var updateCount = 0
    func begin(title: String) { began = true }
    func update(distanceMeters: Double, elapsedSeconds: Int) { updateCount += 1 }
    func end() { ended = true }
}

/// Records the background-location calls a recording makes against the seam.
private final class BackgroundSpyProvider: LocationProvider, @unchecked Sendable {
    private(set) var alwaysAuthorizationRequested = false
    private(set) var backgroundUpdates = false
    func requestAuthorization() {}
    func requestAlwaysAuthorization() { alwaysAuthorizationRequested = true }
    func setBackgroundUpdates(_ enabled: Bool) { backgroundUpdates = enabled }
    func fixes() -> AsyncStream<LocationFix> { AsyncStream { $0.finish() } }
}
