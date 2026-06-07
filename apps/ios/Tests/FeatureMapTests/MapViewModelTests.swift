import Testing
import CoreModel
import CoreData
@testable import FeatureMap

@Suite("MapViewModel")
@MainActor
struct MapViewModelTests {

    @Test("start() loads markers from the repository")
    func loadsMarkers() async {
        let vm = MapViewModel(markerRepository: InMemoryMarkerRepository())
        vm.start()
        try? await Task.sleep(for: .milliseconds(150))
        #expect(vm.markers.count == 3)
        vm.stop()
    }

    @Test("cycleBaseLayer walks all layers and wraps")
    func cycle() {
        let vm = MapViewModel(markerRepository: InMemoryMarkerRepository(seed: []))
        #expect(vm.baseLayer == .norgeskart)
        vm.cycleBaseLayer(); #expect(vm.baseLayer == .osm)
        vm.cycleBaseLayer(); #expect(vm.baseLayer == .satellite)
        vm.cycleBaseLayer(); #expect(vm.baseLayer == .norgeskart)
    }

    @Test("toggleFollowing flips the flag")
    func follow() {
        let vm = MapViewModel(markerRepository: InMemoryMarkerRepository(seed: []))
        vm.toggleFollowing()
        #expect(vm.following)
    }

    @Test("addMarker persists through the repository")
    func addMarker() async {
        let repo = InMemoryMarkerRepository(seed: [])
        let vm = MapViewModel(markerRepository: repo)
        vm.start()
        vm.addMarker(at: LatLng(lat: 69.6, lng: 20.0), kind: .cabin)
        try? await Task.sleep(for: .milliseconds(150))
        #expect(vm.markers.contains { $0.kind == .cabin })
        vm.stop()
    }
}
