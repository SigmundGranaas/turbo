import Testing
@testable import CoreModel

@Suite("CoreModel")
struct CoreModelTests {

    @Test("activity keys round-trip and match the Android contract")
    func activityKeys() {
        #expect(ActivityKindId.fromKey("Fjell") == .mountain)
        #expect(ActivityKindId.fromKey("Fiskeplass") == .fishing)
        #expect(ActivityKindId.fromKey("nope") == nil)
        #expect(ActivityKindId.allCases.count == 18)
    }

    @Test("coordinate formatting matches the design samples")
    func coordFormatting() {
        let p = LatLng(lat: 69.6412, lng: 20.1003)
        #expect(Geo.formatCoords(p) == "69.6412° N, 20.1003° E")
        let s = LatLng(lat: -33.9, lng: -18.4)
        #expect(Geo.formatCoords(s) == "33.9000° S, 18.4000° W")
    }

    @Test("bounds centre is the midpoint")
    func boundsCenter() {
        let b = GeoBounds(south: 0, west: 0, north: 10, east: 20)
        let c = Geo.center(of: b)
        #expect(c.lat == 5)
        #expect(c.lng == 10)
    }

    @Test("base-layer ids match the tile-source contract")
    func layerIds() {
        #expect(BaseLayer.norgeskart.id == "topo")
        #expect(BaseLayer.satellite.id == "gs")
    }
}
