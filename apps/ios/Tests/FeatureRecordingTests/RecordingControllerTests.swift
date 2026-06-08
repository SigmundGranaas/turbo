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
