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

    @Test("following projects live position into progress + remaining distance")
    func progress() async {
        // A fix at the midpoint of the line.
        let provider = SimulatedLocationProvider(fixes: [LocationFix(position: LatLng(lat: 0, lng: 0.018))])
        let follow = FollowController(location: provider)
        follow.start(route)
        #expect(follow.isFollowing)
        try? await Task.sleep(for: .milliseconds(150))
        #expect(abs(follow.fraction - 0.5) < 0.03)
        #expect(follow.distanceRemainingM > 0)
        #expect(follow.etaSeconds != nil)
        #expect(follow.isOffRoute == false)
    }

    @Test("straying past the threshold flags off-route")
    func offRoute() async {
        // ~1 km north of the line.
        let provider = SimulatedLocationProvider(fixes: [LocationFix(position: LatLng(lat: 0.01, lng: 0.018))])
        let follow = FollowController(location: provider)
        follow.start(route)   // no reroute closure
        try? await Task.sleep(for: .milliseconds(150))
        #expect(follow.isOffRoute)
    }

    @Test("arriving within 40 m flips arrived; stop resets")
    func arriveAndStop() async {
        let provider = SimulatedLocationProvider(fixes: [LocationFix(position: LatLng(lat: 0, lng: 0.036))])
        let follow = FollowController(location: provider)
        follow.start(route)
        try? await Task.sleep(for: .milliseconds(150))
        #expect(follow.arrived)
        follow.stop()
        #expect(follow.isFollowing == false)
        #expect(follow.geometry.isEmpty)
    }
}
