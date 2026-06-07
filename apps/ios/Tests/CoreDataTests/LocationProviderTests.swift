import Testing
import CoreModel
@testable import CoreData

@Suite("LocationProvider")
struct LocationProviderTests {

    @Test("the simulated provider streams its scripted fixes")
    func simulatedStreams() async {
        let provider = SimulatedLocationProvider(fixes: [
            LocationFix(position: LatLng(lat: 69.6, lng: 20.0), headingDegrees: 90),
            LocationFix(position: LatLng(lat: 69.61, lng: 20.01), headingDegrees: 95),
        ])
        var received: [LocationFix] = []
        for await fix in provider.fixes() { received.append(fix) }
        #expect(received.count == 2)
        #expect(received.first?.headingDegrees == 90)
        #expect(received.last?.position.lat == 69.61)
    }
}
