import Testing
import Foundation
import CoreModel
@testable import CoreData

@Suite("Trail search")
struct TrailSearchTests {

    private let lineString = """
    { "type": "FeatureCollection", "features": [
      { "type": "Feature",
        "properties": { "navn": "Heggmotinden rundt", "rutenummer": "T-12", "merkemetode": "Malingsmerket" },
        "geometry": { "type": "LineString", "coordinates": [[19.88, 69.55], [19.90, 69.56]] } }
    ] }
    """.data(using: .utf8)!

    private let multiLine = """
    { "type": "FeatureCollection", "features": [
      { "type": "Feature",
        "properties": { "navn": "Storvika sti" },
        "geometry": { "type": "MultiLineString", "coordinates": [[[20.05, 69.62], [20.06, 69.63]]] } }
    ] }
    """.data(using: .utf8)!

    @Test("parses a LineString trail keyed at its first vertex with hiking kind")
    func line() {
        let hits = GeonorgeTrailSearchRepository.parse(lineString)
        #expect(hits.count == 1)
        #expect(hits[0].name == "Heggmotinden rundt")
        #expect(hits[0].kind == .hiking)
        #expect(hits[0].description == "T-12 · Malingsmerket")
        #expect(abs(hits[0].position.lat - 69.55) < 1e-6)
        #expect(abs(hits[0].position.lng - 19.88) < 1e-6)
    }

    @Test("parses a MultiLineString trail at the first vertex of the first line")
    func multi() {
        let hits = GeonorgeTrailSearchRepository.parse(multiLine)
        #expect(hits.count == 1)
        #expect(abs(hits[0].position.lat - 69.62) < 1e-6)
    }

    @Test("malformed / empty feed yields no hits")
    func empty() {
        #expect(GeonorgeTrailSearchRepository.parse(Data("nope".utf8)).isEmpty)
        #expect(GeonorgeTrailSearchRepository.parse(#"{"features":[]}"#.data(using: .utf8)!).isEmpty)
    }

    @Test("composite merges place + trail and de-dups by name")
    func composite() async {
        let place = InMemorySearchRepository(corpus: [
            SearchHit(name: "Heggmotinden", description: "summit", position: LatLng(lat: 69.55, lng: 19.88), kind: .mountain),
        ])
        let trails = InMemoryTrailSearchRepository(hits: [
            SearchHit(name: "Heggmotinden rundt", description: "T-12", position: LatLng(lat: 69.55, lng: 19.88), kind: .hiking),
        ])
        let composite = CompositeSearchRepository(place: place, trails: trails)
        let hits = await composite.search("Heggmotinden")
        #expect(hits.count == 2)                    // both, distinct names
        #expect(hits[0].kind == .mountain)          // place first
        #expect(hits[1].kind == .hiking)
    }
}
