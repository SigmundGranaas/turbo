import Testing
import Foundation
import CoreModel
@testable import CoreData

@Suite("Sunrise parsing")
struct SunProviderTests {

    @Test("parses sunrise and sunset times")
    func normalDay() {
        let body = """
        { "properties": {
            "sunrise": { "time": "2026-06-08T02:11+02:00" },
            "sunset":  { "time": "2026-06-08T22:46+02:00" },
            "solarnoon": { "visible": true }
        } }
        """.data(using: .utf8)!
        let sun = MetNoSunProvider.parse(body)
        #expect(sun?.sunrise != nil)
        #expect(sun?.sunset != nil)
        #expect(sun?.polarDay == false)
        // ~20.5 h of daylight
        #expect((sun?.daylight ?? 0) > 20 * 3600)
    }

    @Test("no sunrise/sunset with visible noon = polar day")
    func polarDay() {
        let body = """
        { "properties": { "solarnoon": { "visible": true } } }
        """.data(using: .utf8)!
        let sun = MetNoSunProvider.parse(body)
        #expect(sun?.polarDay == true)
        #expect(sun?.daylight == TimeInterval(24 * 3600))
    }

    @Test("no sunrise/sunset with non-visible noon = polar night")
    func polarNight() {
        let body = """
        { "properties": { "solarnoon": { "visible": false } } }
        """.data(using: .utf8)!
        let sun = MetNoSunProvider.parse(body)
        #expect(sun?.polarNight == true)
        #expect(sun?.daylight == TimeInterval(0))
    }

    @Test("malformed payload yields nil")
    func malformed() {
        #expect(MetNoSunProvider.parse(Data("x".utf8)) == nil)
        #expect(MetNoSunProvider.parse(#"{"properties":{}}"#.data(using: .utf8)!) == nil)
    }

    @Test("offset string formats the timezone correctly")
    func offset() {
        #expect(MetNoSunProvider.offsetString(7200) == "+02:00")
        #expect(MetNoSunProvider.offsetString(-3600) == "-01:00")
        #expect(MetNoSunProvider.offsetString(0) == "+00:00")
    }
}
