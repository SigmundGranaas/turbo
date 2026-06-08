import Testing
import Foundation
import CoreModel
@testable import CoreData

@Suite("Reverse geocode parsing")
struct ReverseGeocodeTests {

    private let fixture = """
    { "navn": [
        { "skrivemåte": "Storvatnet", "navneobjekttype": "Vatn", "meterFraPunkt": 420.0 },
        { "skrivemåte": "Heggmotinden", "navneobjekttype": "Fjelltopp", "meterFraPunkt": 80.0 },
        { "skrivemåte": "Lyngen", "navneobjekttype": "Bygd", "meterFraPunkt": 950.0 }
    ] }
    """.data(using: .utf8)!

    @Test("picks the nearest place and qualifies a peak as 'On'")
    func nearest() {
        let desc = KartverketReverseGeocodeRepository.describe(fixture)
        #expect(desc?.title == "Heggmotinden")      // closest (80 m)
        #expect(desc?.label == "On Heggmotinden")    // Fjelltopp → On
    }

    @Test("qualifier maps settlement → In, water → At, else → Near")
    func qualifiers() {
        #expect(KartverketReverseGeocodeRepository.qualifier(for: "Bygd") == .in)
        #expect(KartverketReverseGeocodeRepository.qualifier(for: "Vatn") == .at)
        #expect(KartverketReverseGeocodeRepository.qualifier(for: "Myr") == .near)
    }

    @Test("empty / malformed payload yields nil")
    func empty() {
        #expect(KartverketReverseGeocodeRepository.describe(Data("x".utf8)) == nil)
        #expect(KartverketReverseGeocodeRepository.describe(#"{"navn":[]}"#.data(using: .utf8)!) == nil)
    }
}
