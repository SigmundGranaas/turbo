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

    @Test("remove a specific waypoint by index")
    func removeSpecific() {
        let (vm, _) = vm()
        vm.setMode(.line)
        [LatLng(lat: 1, lng: 1), LatLng(lat: 2, lng: 2), LatLng(lat: 3, lng: 3)].forEach(vm.addWaypoint)
        #expect(vm.waypoints.count == 3)
        vm.removeWaypoint(at: 1)
        #expect(vm.waypoints.map(\.lat) == [1, 3])   // middle dropped
    }

    @Test("reorder waypoints")
    func reorder() {
        let (vm, _) = vm()
        vm.setMode(.line)
        [LatLng(lat: 1, lng: 1), LatLng(lat: 2, lng: 2), LatLng(lat: 3, lng: 3)].forEach(vm.addWaypoint)
        vm.moveWaypoint(from: 0, to: 2)
        #expect(vm.waypoints.map(\.lat) == [2, 3, 1])
    }

    @Test("drag a waypoint to a new position")
    func dragMove() {
        let (vm, _) = vm()
        vm.setMode(.line)
        vm.addWaypoint(LatLng(lat: 1, lng: 1))
        vm.addWaypoint(LatLng(lat: 2, lng: 2))
        vm.moveWaypoint(at: 0, to: LatLng(lat: 9, lng: 9))
        #expect(vm.waypoints.first?.lat == 9)
    }

    @Test("insert a stop at the least-detour segment")
    func insertLeastDetour() {
        // A→C with B near the A→C line: inserting D near A→B lands between A and B.
        let a = LatLng(lat: 0, lng: 0), b = LatLng(lat: 0, lng: 2), c = LatLng(lat: 0, lng: 4)
        let result = RouteViewModel.insertLeastDetour([a, b, c], LatLng(lat: 0, lng: 1))
        #expect(result.map(\.lng) == [0, 1, 2, 4])
    }

    @Test("multi-level undo reverts edits one at a time")
    func undo() {
        let (vm, _) = vm()
        vm.setMode(.line)
        vm.addWaypoint(LatLng(lat: 1, lng: 1))   // undo→[]
        vm.addWaypoint(LatLng(lat: 2, lng: 2))   // undo→[1]
        #expect(vm.canUndo)
        vm.undo()
        #expect(vm.waypoints.map(\.lat) == [1])
        vm.undo()
        #expect(vm.waypoints.isEmpty)
        #expect(vm.canUndo == false)
    }

    @Test("draw mode captures a freehand stroke and measures it")
    func draw() {
        let (vm, _) = vm()
        vm.setMode(.draw)
        vm.beginStroke()
        vm.appendDrawPoint(LatLng(lat: 0, lng: 0))
        vm.appendDrawPoint(LatLng(lat: 0, lng: 0.01))   // ~1.1 km east
        #expect(vm.drawPoints.count == 2)
        #expect(vm.geometry.count == 2)
        #expect((vm.plan?.distanceM ?? 0) > 0)
        // Taps are ignored in draw mode.
        vm.addWaypoint(LatLng(lat: 5, lng: 5))
        #expect(vm.waypoints.isEmpty)
    }

    @Test("draw throttles near-duplicate points")
    func drawThrottle() {
        let (vm, _) = vm()
        vm.setMode(.draw)
        vm.beginStroke()
        vm.appendDrawPoint(LatLng(lat: 69.6, lng: 19.9))
        vm.appendDrawPoint(LatLng(lat: 69.6, lng: 19.9))   // identical → dropped
        #expect(vm.drawPoints.count == 1)
    }

    @Test("switching modes starts a fresh track")
    func modeSwitchClears() {
        let (vm, _) = vm()
        vm.setMode(.line)
        vm.addWaypoint(LatLng(lat: 1, lng: 1))
        vm.addWaypoint(LatLng(lat: 2, lng: 2))
        #expect(vm.waypoints.count == 2)
        vm.setMode(.draw)
        #expect(vm.waypoints.isEmpty)
        #expect(vm.plan == nil)
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
