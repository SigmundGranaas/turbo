import Testing
@testable import CoreModel

@Suite("Follow-mode progress")
struct JourneyProgressTests {

    // A 4 km straight east-west line at the equator (1° lng ≈ 111 km).
    private let route = [LatLng(lat: 0, lng: 0), LatLng(lat: 0, lng: 0.036)]  // ~4 km

    @Test("eta uses Naismith: 1h/5km flat + 1h/600m climb")
    func eta() {
        #expect(GeoMetrics.etaSeconds(distanceM: 5000, ascentM: nil) == 3600)
        #expect(GeoMetrics.etaSeconds(distanceM: 0, ascentM: 600) == 3600)
        #expect(GeoMetrics.etaSeconds(distanceM: 5000, ascentM: 600) == 7200)
    }

    @Test("progress: halfway along the line is ~50% with half the distance left")
    func halfway() {
        let p = GeoMetrics.progress(route, position: LatLng(lat: 0, lng: 0.018))
        #expect(p != nil)
        #expect(abs((p?.fraction ?? 0) - 0.5) < 0.02)
        let total = GeoMetrics.pathLengthMeters(route)
        #expect(abs((p?.distanceRemainingM ?? 0) - total / 2) < total * 0.02)
    }

    @Test("progress: start is 0%, end is 100%")
    func ends() {
        #expect((GeoMetrics.progress(route, position: route[0])?.fraction ?? 1) < 0.01)
        #expect((GeoMetrics.progress(route, position: route[1])?.fraction ?? 0) > 0.99)
    }

    @Test("distanceToPath is ~0 on the line, large when off it")
    func offRoute() {
        #expect(GeoMetrics.distanceToPath(route, LatLng(lat: 0, lng: 0.018)) < 5)
        // ~0.01° north of the line ≈ 1.1 km away
        #expect(GeoMetrics.distanceToPath(route, LatLng(lat: 0.01, lng: 0.018)) > 500)
    }

    @Test("a degenerate route yields no progress")
    func degenerate() {
        #expect(GeoMetrics.progress([LatLng(lat: 1, lng: 1)], position: LatLng(lat: 1, lng: 1)) == nil)
    }
}
