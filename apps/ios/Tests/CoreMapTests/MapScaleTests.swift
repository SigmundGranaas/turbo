import Testing
@testable import CoreMap

@Suite("MapScale")
struct MapScaleTests {

    @Test("picks the largest round distance that fits the bar width")
    func picksRoundDistance() {
        // 10 m/pt over an 80pt budget = 800 m max → 500 m bar.
        let a = MapScale.bar(metersPerPoint: 10, maxWidthPoints: 80)
        #expect(a.label == "500 m")
        #expect(a.widthPoints == 50)   // 500 / 10

        // 1 m/pt, 80pt → 80 m max → 50 m.
        #expect(MapScale.bar(metersPerPoint: 1, maxWidthPoints: 80).label == "50 m")
    }

    @Test("switches to kilometres above 1000 m")
    func kilometres() {
        let k = MapScale.bar(metersPerPoint: 100, maxWidthPoints: 80)  // 8000 m max → 5 km
        #expect(k.label == "5 km")
        #expect(k.widthPoints == 50)
    }

    @Test("degenerate inputs don't crash and stay positive")
    func degenerate() {
        let z = MapScale.bar(metersPerPoint: 0, maxWidthPoints: 80)
        #expect(z.widthPoints >= 0)
        #expect(!z.label.isEmpty)
    }
}
