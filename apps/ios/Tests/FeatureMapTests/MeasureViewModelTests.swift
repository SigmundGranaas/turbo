import Testing
import CoreModel
@testable import FeatureMap

@Suite("MeasureViewModel")
@MainActor
struct MeasureViewModelTests {

    @Test("distance accumulates as points are added")
    func distance() {
        let vm = MeasureViewModel()
        #expect(vm.distanceMeters == 0)
        vm.addPoint(LatLng(lat: 69.60, lng: 20.00))
        #expect(vm.distanceMeters == 0)        // one point = no length
        vm.addPoint(LatLng(lat: 69.61, lng: 20.02))
        #expect(vm.distanceMeters > 0)
    }

    @Test("undo + clear reset the measurement")
    func reset() {
        let vm = MeasureViewModel()
        vm.addPoint(LatLng(lat: 1, lng: 2))
        vm.addPoint(LatLng(lat: 3, lng: 4))
        vm.removeLast()
        #expect(vm.points.count == 1)
        #expect(vm.distanceMeters == 0)
        vm.clear()
        #expect(vm.points.isEmpty)
    }
}
