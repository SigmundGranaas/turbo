import Testing
import CoreModel

@Suite("GeoMetrics route split")
struct RouteSplitTests {
    // A 2-unit-long straight line.
    private let route = [LatLng(lat: 0, lng: 0), LatLng(lat: 0, lng: 1), LatLng(lat: 0, lng: 2)]

    @Test("routePrefix is the covered part up to the cursor")
    func prefix() {
        #expect(GeoMetrics.routePrefix(route, 0).isEmpty)
        #expect(GeoMetrics.routePrefix(route, 1) == route)
        let half = GeoMetrics.routePrefix(route, 0.5)
        #expect(half.count == 2)
        #expect(half.first?.lng == 0)
        #expect(abs((half.last?.lng ?? 0) - 1) < 0.02)
    }

    @Test("routeSuffix complements routePrefix and they meet at the cursor")
    func suffix() {
        #expect(GeoMetrics.routeSuffix(route, 0) == route)
        #expect(GeoMetrics.routeSuffix(route, 1).isEmpty)
        let prefix = GeoMetrics.routePrefix(route, 0.5)
        let suffix = GeoMetrics.routeSuffix(route, 0.5)
        #expect(prefix.last?.lng == suffix.first?.lng)
        #expect(prefix.last?.lat == suffix.first?.lat)
        #expect(abs((suffix.last?.lng ?? 0) - 2) < 1e-9)
    }

    @Test("arcLengthAlong returns where a point projects along the route")
    func arcLength() {
        let total = GeoMetrics.pathLengthMeters(route)
        #expect(GeoMetrics.arcLengthAlong(route, LatLng(lat: 0, lng: 0)) < 1)
        #expect(abs(GeoMetrics.arcLengthAlong(route, LatLng(lat: 0, lng: 2)) - total) < 1)
        #expect(abs(GeoMetrics.arcLengthAlong(route, LatLng(lat: 0, lng: 1)) - total / 2) < 1)
        // An off-route point projects to its nearest along-track position, not 0.
        let off = GeoMetrics.arcLengthAlong(route, LatLng(lat: 0.0005, lng: 1))
        #expect(abs(off - total / 2) < 200)
    }
}
