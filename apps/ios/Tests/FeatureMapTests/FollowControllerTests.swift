import Testing
import CoreModel
import CoreData
@testable import FeatureMap

@Suite("FollowController")
@MainActor
struct FollowControllerTests {

    // ~4 km west→east line near the equator.
    private let route = FollowRoute(
        geometry: [LatLng(lat: 0, lng: 0), LatLng(lat: 0, lng: 0.036)],
        distanceM: 4000, ascentM: 0, name: "Test", waypoints: []
    )

    /// A provider that walks a sequence of on/near-route fixes (the cursor needs
    /// to be *walked* — it can't teleport to a far point in one fix).
    private func provider(_ points: [(lat: Double, lng: Double)], altitude: Double? = nil) -> SimulatedLocationProvider {
        SimulatedLocationProvider(
            fixes: points.map { LocationFix(position: LatLng(lat: $0.lat, lng: $0.lng), altitude: altitude) },
            interval: .milliseconds(4)
        )
    }

    @Test("walking the route advances fraction + remaining distance")
    func progress() async {
        // Walk to the midpoint in 333 m steps (inside the cursor window).
        let lngs = (0...6).map { (0.0, Double($0) * 0.003) }   // 0 … 0.018
        let follow = FollowController(location: provider(lngs), pathRepository: InMemoryPathRepository(seed: []))
        follow.start(route)
        #expect(follow.isFollowing)
        try? await Task.sleep(for: .milliseconds(250))
        #expect(abs(follow.fraction - 0.5) < 0.03)
        #expect(follow.distanceRemainingM > 0)
        #expect(follow.etaSeconds != nil)
        #expect(follow.isOffRoute == false)
    }

    @Test("straying ~1 km off the line for several fixes flags off-route")
    func offRoute() async {
        // Three consecutive fixes ~1.1 km north of the route (debounced off-route).
        let follow = FollowController(location: provider([(0.01, 0.0), (0.01, 0.001), (0.01, 0.002)]), pathRepository: InMemoryPathRepository(seed: []))
        follow.start(route)   // no reroute closure
        try? await Task.sleep(for: .milliseconds(200))
        #expect(follow.isOffRoute)
    }

    @Test("walking to the end flips arrived; stop resets")
    func arriveAndStop() async {
        let lngs = (0...12).map { (0.0, Double($0) * 0.003) }   // 0 … 0.036 (the end)
        let follow = FollowController(location: provider(lngs), pathRepository: InMemoryPathRepository(seed: []))
        follow.start(route)
        try? await Task.sleep(for: .milliseconds(300))
        #expect(follow.arrived)
        follow.stop()
        #expect(follow.isFollowing == false)
        #expect(follow.geometry.isEmpty)
    }

    @Test("following captures the real travelled track (Follow = Record)")
    func capturesTrack() async {
        let lngs = (0...8).map { (0.0, Double($0) * 0.003) }
        let follow = FollowController(location: provider(lngs, altitude: 100), pathRepository: InMemoryPathRepository(seed: []))
        follow.start(route)
        try? await Task.sleep(for: .milliseconds(300))
        #expect(follow.capturedDistanceM > 1500)
    }

    @Test("finishing a real follow auto-saves the travelled track")
    func autoSaves() async {
        let lngs = (0...8).map { (0.0, Double($0) * 0.003) }
        let repo = InMemoryPathRepository(seed: [])
        let follow = FollowController(location: provider(lngs, altitude: 100), pathRepository: repo)
        follow.start(route)
        try? await Task.sleep(for: .milliseconds(300))
        follow.stop()
        try? await Task.sleep(for: .milliseconds(150))
        let saved = await repo.current()
        #expect(saved.count == 1)
        #expect(saved.first?.path.source == .recording)
        #expect((saved.first?.name ?? "").contains("Test"))
        #expect((saved.first?.path.points.count ?? 0) >= 8)
    }

    @Test("a trivially short follow is not auto-saved")
    func skipsTrivial() async {
        // Two fixes ~22 m apart — under the 50 m save floor.
        let repo = InMemoryPathRepository(seed: [])
        let follow = FollowController(location: provider([(0.0, 0.0), (0.0, 0.0002)]), pathRepository: repo)
        follow.start(route)
        try? await Task.sleep(for: .milliseconds(100))
        follow.stop()
        try? await Task.sleep(for: .milliseconds(100))
        let saved = await repo.current()
        #expect(saved.isEmpty)
    }
}
