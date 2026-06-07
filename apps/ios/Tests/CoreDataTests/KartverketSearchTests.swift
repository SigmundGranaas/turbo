import Testing
import Foundation
import CoreModel
@testable import CoreData

@Suite("Kartverket search parsing")
struct KartverketSearchTests {

    /// A trimmed real-shape response from ws.geonorge.no stedsnavn (note the
    /// Norwegian field names with å / ø).
    private let fixture = """
    {
      "navn": [
        {
          "skrivemåte": "Heggmotinden",
          "navneobjekttype": "Fjell",
          "kommuner": [{ "kommunenavn": "Tromsø" }],
          "representasjonspunkt": { "øst": 19.8801, "nord": 69.5502, "koordsys": 4258 }
        },
        {
          "skrivemåte": "Storvikelva",
          "navneobjekttype": "Elv",
          "kommuner": [{ "kommunenavn": "Senja" }],
          "representasjonspunkt": { "øst": 20.12, "nord": 69.60, "koordsys": 4258 }
        }
      ],
      "metadata": { "totaltAntallTreff": 2 }
    }
    """.data(using: .utf8)!

    @Test("decodes stedsnavn into SearchHits with name, type+kommune, position")
    func decode() throws {
        let hits = try KartverketSearchRepository.decodeHits(from: fixture)
        #expect(hits.count == 2)
        #expect(hits[0].name == "Heggmotinden")
        #expect(hits[0].description.contains("Fjell"))
        #expect(hits[0].description.contains("Tromsø"))
        #expect(abs(hits[0].position.lat - 69.5502) < 0.0001)
        #expect(abs(hits[0].position.lng - 19.8801) < 0.0001)
    }

    @Test("maps known place types to an activity kind")
    func kindMapping() throws {
        let hits = try KartverketSearchRepository.decodeHits(from: fixture)
        #expect(hits[0].kind == .mountain)   // Fjell → mountain
    }

    @Test("an empty result set decodes to no hits")
    func empty() throws {
        let data = #"{ "navn": [], "metadata": {} }"#.data(using: .utf8)!
        #expect(try KartverketSearchRepository.decodeHits(from: data).isEmpty)
    }
}
