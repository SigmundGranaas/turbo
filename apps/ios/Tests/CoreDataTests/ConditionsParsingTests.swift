import Testing
import Foundation
import CoreModel
@testable import CoreData

@Suite("met.no weather parsing")
struct MetNoParsingTests {

    private let fixture = """
    { "properties": { "timeseries": [
        { "time": "2026-06-08T12:00:00Z", "data": {
            "instant": { "details": { "air_temperature": -3.4 } },
            "next_1_hours": { "summary": { "symbol_code": "lightsnow" } },
            "next_6_hours": { "summary": { "symbol_code": "snow" } } } },
        { "time": "2026-06-08T13:00:00Z", "data": {
            "instant": { "details": { "air_temperature": -2.0 } },
            "next_1_hours": { "summary": { "symbol_code": "partlycloudy_day" } } } },
        { "time": "2026-06-09T12:00:00Z", "data": {
            "instant": { "details": { "air_temperature": 1.0 } },
            "next_1_hours": { "summary": { "symbol_code": "clearsky_day" } } } }
    ] } }
    """.data(using: .utf8)!

    @Test("parses current temp + symbol, hourly and daily")
    func parses() {
        let s = MetNoWeatherProvider.parse(fixture, placeName: "Lyngen")
        #expect(s != nil)
        #expect(s?.temperatureC == -3.4)
        #expect(s?.symbol == .snow)              // lightsnow → snow
        #expect(s?.hourly.first?.label == "Now")
        #expect(s?.hourly.count == 3)
        #expect(s?.daily.count == 2)             // two distinct days
        #expect(s?.daily.first?.weekday == "Today")
    }

    @Test("symbol codes map to kinds")
    func symbols() {
        #expect(MetNoWeatherProvider.symbol("partlycloudy_night") == .partlyCloudy)
        #expect(MetNoWeatherProvider.symbol("heavyrainshowers_day") == .rain)
        #expect(MetNoWeatherProvider.symbol("clearsky_day") == .clear)
        #expect(MetNoWeatherProvider.symbol(nil) == .cloudy)
    }

    @Test("malformed / empty payload yields nil (no data, not fake)")
    func empty() {
        #expect(MetNoWeatherProvider.parse(Data("nope".utf8), placeName: "x") == nil)
        #expect(MetNoWeatherProvider.parse(#"{"properties":{"timeseries":[]}}"#.data(using: .utf8)!, placeName: "x") == nil)
    }
}

@Suite("Varsom avalanche parsing")
struct VarsomParsingTests {

    @Test("parses the first real danger level")
    func parses() {
        let json = """
        [ { "DangerLevel": "3", "MainText": "Wind slabs on NE aspects.", "RegionName": "Lyngen" } ]
        """.data(using: .utf8)!
        let info = VarsomAvalancheProvider.parse(json)
        #expect(info?.level == 3)
        #expect(info?.region == "Lyngen")
        #expect(info?.headline.contains("Wind slabs") == true)
    }

    @Test("level 0 / empty means no warning (nil)")
    func noWarning() {
        #expect(VarsomAvalancheProvider.parse("[]".data(using: .utf8)!) == nil)
        #expect(VarsomAvalancheProvider.parse(#"[{"DangerLevel":"0"}]"#.data(using: .utf8)!) == nil)
        #expect(VarsomAvalancheProvider.parse(Data("nope".utf8)) == nil)
    }
}
