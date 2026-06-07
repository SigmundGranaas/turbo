import Testing
import CoreModel
import CoreData
@testable import FeatureRecording

@Suite("RecordingViewModel")
@MainActor
struct RecordingViewModelTests {

    private func provider() -> SimulatedLocationProvider {
        SimulatedLocationProvider(fixes: [
            LocationFix(position: LatLng(lat: 69.60, lng: 19.90), altitude: 10),
            LocationFix(position: LatLng(lat: 69.61, lng: 19.92), altitude: 24),
            LocationFix(position: LatLng(lat: 69.62, lng: 19.95), altitude: 18),
        ])
    }

    @Test("recording accumulates points + distance from location fixes")
    func accumulates() async {
        let vm = RecordingViewModel(location: provider(), pathRepository: InMemoryPathRepository(seed: []))
        vm.start()
        #expect(vm.isRecording)
        try? await Task.sleep(for: .milliseconds(250))
        #expect(vm.pointCount == 3)
        #expect(vm.distanceMeters > 0)
    }

    @Test("stop keeps the captured track; save persists it as a SavedPath")
    func saveTrack() async {
        let repo = InMemoryPathRepository(seed: [])
        let vm = RecordingViewModel(location: provider(), pathRepository: repo)
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
        let vm = RecordingViewModel(location: provider(), pathRepository: repo)
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
        let vm = RecordingViewModel(location: provider(), pathRepository: repo)
        vm.start()
        try? await Task.sleep(for: .milliseconds(250))
        vm.stop()
        vm.save(name: "   ")
        try? await Task.sleep(for: .milliseconds(150))
        #expect(await repo.current().first?.name.isEmpty == false)
    }
}
