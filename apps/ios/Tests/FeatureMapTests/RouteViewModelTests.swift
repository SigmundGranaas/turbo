import Testing
import CoreModel
import CoreData
@testable import FeatureMap

@Suite("RouteViewModel")
@MainActor
struct RouteViewModelTests {

    private func vm(_ route: RouteRepository = InMemoryRouteRepository()) -> (RouteViewModel, InMemoryPathRepository) {
        let paths = InMemoryPathRepository(seed: [])
        return (RouteViewModel(routeRepository: route, pathRepository: paths), paths)
    }

    @Test("line mode connects waypoints with straight legs")
    func line() {
        let (vm, _) = vm()
        vm.setMode(.line)
        vm.addWaypoint(LatLng(lat: 69.6, lng: 20.0))
        vm.addWaypoint(LatLng(lat: 69.7, lng: 20.1))
        #expect(vm.geometry.count == 2)
        #expect(vm.plan?.distanceM ?? 0 > 0)
    }

    @Test("route mode solves via the repository and sets a plan")
    func route() async {
        let (vm, _) = vm()
        vm.setMode(.route)
        vm.addWaypoint(LatLng(lat: 69.6, lng: 20.0))
        vm.addWaypoint(LatLng(lat: 69.7, lng: 20.1))
        try? await Task.sleep(for: .milliseconds(150))
        #expect(vm.plan != nil)
        #expect(vm.geometry.count == 2)
        #expect(vm.isSolving == false)
    }

    @Test("a single waypoint has no plan; removeLast + clear reset")
    func resets() {
        let (vm, _) = vm()
        vm.setMode(.line)
        vm.addWaypoint(LatLng(lat: 1, lng: 2))
        #expect(vm.plan == nil)
        vm.addWaypoint(LatLng(lat: 3, lng: 4))
        #expect(vm.plan != nil)
        vm.removeLast()
        #expect(vm.plan == nil)
        vm.clear()
        #expect(vm.waypoints.isEmpty)
        #expect(vm.geometry.isEmpty)
    }

    @Test("saving a solved route persists it as a recorded path")
    func save() async {
        let (vm, paths) = vm()
        vm.setMode(.line)
        vm.addWaypoint(LatLng(lat: 69.6, lng: 20.0))
        vm.addWaypoint(LatLng(lat: 69.7, lng: 20.1))
        vm.saveAsPath(name: "Summit loop")
        try? await Task.sleep(for: .milliseconds(120))
        let saved = await paths.current()
        #expect(saved.first?.name == "Summit loop")
        #expect(saved.first?.path.source == .route)
        #expect(saved.first?.path.points.count == 2)
    }
}
