import Testing
import Foundation
import CoreModel
@testable import CoreData

@Suite("Marine forecast parsing")
struct MarineProviderTests {

    @Test("parses the current marine point")
    func current() {
        let body = """
        { "properties": { "timeseries": [
            { "time": "2026-06-08T12:00:00Z",
              "data": { "instant": { "details": {
                "sea_surface_wave_height": 1.4,
                "sea_water_temperature": 11.2,
                "sea_water_speed": 0.3
              } } } },
            { "time": "2026-06-08T13:00:00Z",
              "data": { "instant": { "details": { "sea_surface_wave_height": 99.0 } } } }
        ] } }
        """.data(using: .utf8)!
        let m = MetNoMarineProvider.parse(body)
        #expect(m?.waveHeightM == 1.4)        // first point only
        #expect(m?.seaTemperatureC == 11.2)
        #expect(m?.seaCurrentMs == 0.3)
    }

    @Test("inland / empty timeseries yields nil")
    func empty() {
        #expect(MetNoMarineProvider.parse(Data("x".utf8)) == nil)
        #expect(MetNoMarineProvider.parse(#"{"properties":{"timeseries":[]}}"#.data(using: .utf8)!) == nil)
    }

    @Test("a point with no marine details yields nil")
    func noDetails() {
        let body = """
        { "properties": { "timeseries": [
            { "time": "2026-06-08T12:00:00Z", "data": { "instant": { "details": {} } } }
        ] } }
        """.data(using: .utf8)!
        #expect(MetNoMarineProvider.parse(body) == nil)
    }
}
