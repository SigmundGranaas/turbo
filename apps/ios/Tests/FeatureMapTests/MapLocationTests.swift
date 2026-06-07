import Testing
import CoreModel
import CoreData
@testable import FeatureMap

@Suite("Map location + heading")
@MainActor
struct MapLocationTests {

    @Test("enabling location streams the user's position and heading into state")
    func enableLocation() async {
        let provider = SimulatedLocationProvider(fixes: [
            LocationFix(position: LatLng(lat: 69.60, lng: 20.00), headingDegrees: 45),
        ])
        let vm = MapViewModel(markerRepository: InMemoryMarkerRepository(seed: []), location: provider)
        #expect(vm.userLocation == nil)
        vm.enableLocation()
        try? await Task.sleep(for: .milliseconds(200))
        #expect(vm.userLocation?.lat == 69.60)
        #expect(vm.heading == 45)
        vm.stop()
    }

    @Test("enableLocation is idempotent (no duplicate observers)")
    func idempotent() async {
        let provider = SimulatedLocationProvider(fixes: [
            LocationFix(position: LatLng(lat: 1, lng: 2), headingDegrees: nil),
        ])
        let vm = MapViewModel(markerRepository: InMemoryMarkerRepository(seed: []), location: provider)
        vm.enableLocation()
        vm.enableLocation()
        try? await Task.sleep(for: .milliseconds(150))
        #expect(vm.userLocation?.lat == 1)
        vm.stop()
    }
}
